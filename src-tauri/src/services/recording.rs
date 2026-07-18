use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::Mutex,
};

use crate::types::{TableScribeError, TableScribeResult};

pub const TARGET_SAMPLE_RATE: u32 = 16_000;
pub const TARGET_CHANNELS: u16 = 1;
pub const TARGET_BITS_PER_SAMPLE: u16 = 16;

pub const WAV_HEADER_LEN: u64 = 44;
const RIFF_OVERHEAD: u64 = 36;
const MAX_DATA_BYTES: u64 = u32::MAX as u64 - RIFF_OVERHEAD;

pub struct ActiveRecording {
    writer: BufWriter<File>,
    path: PathBuf,
    campaign_id: String,
    session_name: String,
    data_bytes: u64,
}

#[derive(Default)]
pub struct RecordingState {
    pub inner: Mutex<Option<ActiveRecording>>,
    pub live: Mutex<Option<crate::services::live_transcribe::LiveTranscribeWorker>>,
}

impl ActiveRecording {
    pub fn create(
        path: PathBuf,
        campaign_id: String,
        session_name: String,
    ) -> TableScribeResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;
        let mut writer = BufWriter::new(file);
        write_placeholder_header(&mut writer)?;
        Ok(Self {
            writer,
            path,
            campaign_id,
            session_name,
            data_bytes: 0,
        })
    }

    pub fn append(&mut self, chunk: &[u8]) -> TableScribeResult<()> {
        let next_total = self.data_bytes.saturating_add(chunk.len() as u64);
        if next_total > MAX_DATA_BYTES {
            return Err(TableScribeError::Message(format!(
                "Recording would exceed the WAV size limit ({} bytes). Stop the current session and start a new one.",
                MAX_DATA_BYTES
            )));
        }
        self.writer.write_all(chunk)?;
        self.data_bytes = next_total;
        Ok(())
    }

    /// Force any buffered PCM out to disk so a concurrent read-only descriptor
    /// sees a coherent prefix. Used by the live-transcribe worker before it
    /// reads the trailing window, and by the concurrent-read smoke test.
    #[allow(dead_code)] // The live worker reads on-disk-only today; this is the hook for a future precise-read mode.
    pub fn flush(&mut self) -> TableScribeResult<()> {
        self.writer.flush()?;
        Ok(())
    }

    pub fn finalize(mut self) -> TableScribeResult<(PathBuf, String, String)> {
        self.writer.flush()?;
        let mut file = self.writer.into_inner().map_err(|err| {
            TableScribeError::Message(format!("WAV writer error: {}", err.error()))
        })?;
        rewrite_sizes(&mut file, self.data_bytes)?;
        file.flush()?;
        Ok((self.path, self.campaign_id, self.session_name))
    }

    pub fn discard(mut self) -> TableScribeResult<()> {
        let _ = self.writer.flush();
        drop(self.writer);
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}

fn write_placeholder_header<W: Write>(writer: &mut W) -> TableScribeResult<()> {
    let byte_rate =
        TARGET_SAMPLE_RATE * TARGET_CHANNELS as u32 * TARGET_BITS_PER_SAMPLE as u32 / 8;
    let block_align = TARGET_CHANNELS * TARGET_BITS_PER_SAMPLE / 8;
    writer.write_all(b"RIFF")?;
    writer.write_all(&0u32.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?;
    writer.write_all(&1u16.to_le_bytes())?;
    writer.write_all(&TARGET_CHANNELS.to_le_bytes())?;
    writer.write_all(&TARGET_SAMPLE_RATE.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&TARGET_BITS_PER_SAMPLE.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&0u32.to_le_bytes())?;
    debug_assert_eq!(WAV_HEADER_LEN, 44);
    Ok(())
}

fn rewrite_sizes(file: &mut File, data_bytes: u64) -> TableScribeResult<()> {
    let riff_size = (RIFF_OVERHEAD + data_bytes) as u32;
    let data_size = data_bytes as u32;
    file.seek(SeekFrom::Start(4))?;
    file.write_all(&riff_size.to_le_bytes())?;
    file.seek(SeekFrom::Start(40))?;
    file.write_all(&data_size.to_le_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    struct ParsedWav {
        riff_size: u32,
        fmt_size: u32,
        audio_format: u16,
        channels: u16,
        sample_rate: u32,
        bits_per_sample: u16,
        data_size: u32,
        data: Vec<u8>,
    }

    fn parse_wav(bytes: &[u8]) -> ParsedWav {
        assert_eq!(&bytes[0..4], b"RIFF");
        let riff_size = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        let fmt_size = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let audio_format = u16::from_le_bytes(bytes[20..22].try_into().unwrap());
        let channels = u16::from_le_bytes(bytes[22..24].try_into().unwrap());
        let sample_rate = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let bits_per_sample = u16::from_le_bytes(bytes[34..36].try_into().unwrap());
        assert_eq!(&bytes[36..40], b"data");
        let data_size = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
        let data = bytes[44..].to_vec();
        ParsedWav {
            riff_size,
            fmt_size,
            audio_format,
            channels,
            sample_rate,
            bits_per_sample,
            data_size,
            data,
        }
    }

    fn temp_path(label: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "tablescribe-test-{}-{}-{}.wav",
            label,
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        path
    }

    #[test]
    fn writes_and_finalizes_a_valid_wav() {
        let path = temp_path("roundtrip");
        let mut rec =
            ActiveRecording::create(path.clone(), "c1".into(), "session-001".into()).unwrap();

        let silence = vec![0u8; 16_000 * 2 * 2];
        rec.append(&silence).unwrap();
        let pulse: Vec<u8> = (0..100).map(|i| (i as u8).wrapping_mul(7)).collect();
        rec.append(&pulse).unwrap();
        let total_data = silence.len() + pulse.len();

        let (out_path, campaign_id, session_name) = rec.finalize().unwrap();
        assert_eq!(out_path, path);
        assert_eq!(campaign_id, "c1");
        assert_eq!(session_name, "session-001");

        let mut bytes = Vec::new();
        File::open(&path).unwrap().read_to_end(&mut bytes).unwrap();
        std::fs::remove_file(&path).ok();

        let parsed = parse_wav(&bytes);
        assert_eq!(parsed.audio_format, 1);
        assert_eq!(parsed.channels, TARGET_CHANNELS);
        assert_eq!(parsed.sample_rate, TARGET_SAMPLE_RATE);
        assert_eq!(parsed.bits_per_sample, TARGET_BITS_PER_SAMPLE);
        assert_eq!(parsed.fmt_size, 16);
        assert_eq!(parsed.data_size as usize, total_data);
        assert_eq!(parsed.riff_size as usize, 36 + total_data);
        assert_eq!(bytes.len(), 44 + total_data);
        assert_eq!(&parsed.data[parsed.data.len() - pulse.len()..], &pulse[..]);
    }

    #[test]
    fn buffered_writes_survive_finalize_seek() {
        // A header rewrite that skips flushing the BufWriter would clobber unwritten PCM bytes.
        // Append less than the BufWriter capacity so the bytes sit in the buffer at finalize time.
        let path = temp_path("flush");
        let mut rec =
            ActiveRecording::create(path.clone(), "c".into(), "s".into()).unwrap();
        let small: Vec<u8> = (0..512).map(|i| i as u8).collect();
        rec.append(&small).unwrap();
        let _ = rec.finalize().unwrap();

        let mut bytes = Vec::new();
        File::open(&path).unwrap().read_to_end(&mut bytes).unwrap();
        std::fs::remove_file(&path).ok();

        let parsed = parse_wav(&bytes);
        assert_eq!(parsed.data_size as usize, small.len());
        assert_eq!(parsed.data, small);
    }

    #[test]
    fn discard_removes_file() {
        let path = temp_path("discard");
        let rec = ActiveRecording::create(path.clone(), "c".into(), "s".into()).unwrap();
        rec.discard().unwrap();
        assert!(!path.exists());
    }

    // The live-transcribe worker (Chunk 2) opens a second read-only descriptor on
    // the still-being-recorded WAV to extract a trailing window. This test pins
    // that assumption: a flushed prefix is visible to an independent reader
    // before the writer's BufWriter has been finalized.
    #[test]
    fn concurrent_reader_sees_flushed_prefix() {
        use std::io::{Read, Seek, SeekFrom};

        let path = temp_path("concurrent-read");
        let mut rec =
            ActiveRecording::create(path.clone(), "c".into(), "s".into()).unwrap();
        let payload: Vec<u8> = (0..4096).map(|i| (i as u8).wrapping_mul(11)).collect();
        rec.append(&payload).unwrap();
        rec.flush().unwrap();

        // Open a fresh read-only descriptor while the writer is still alive.
        let mut reader = File::open(&path).unwrap();
        reader.seek(SeekFrom::Start(WAV_HEADER_LEN)).unwrap();
        let mut seen = vec![0u8; payload.len()];
        reader.read_exact(&mut seen).unwrap();
        assert_eq!(seen, payload);

        // The writer is still live and can keep appending.
        let more: Vec<u8> = (0..512).map(|i| !(i as u8)).collect();
        rec.append(&more).unwrap();

        let _ = rec.finalize().unwrap();
        std::fs::remove_file(&path).ok();
    }

    // Sixty-minute synthetic capture. Run with:
    //   cargo test --release -p tablescribe -- --ignored sixty_minute
    //
    // Tiles the chunk1-smoke.wav PCM payload (real 14.79 s capture) up to
    // 3600 s, feeds it through ActiveRecording in ~8 KB chunks at the
    // worklet's 250 ms cadence (~14,400 appends). At this scale the
    // BufWriter has already flushed many times before finalize, so the
    // seek+rewrite of the RIFF/data sizes is touching a mostly-on-disk
    // file — the existing buffered_writes_survive_finalize_seek test only
    // covers the under-capacity case.
    #[test]
    #[ignore]
    fn sixty_minute_synthetic_capture_finalizes_cleanly() {
        const SECONDS: u32 = 3600;
        const CHUNK_BYTES: usize = 8_000; // ~250 ms of 16 kHz mono Int16
        let byte_rate = (TARGET_SAMPLE_RATE
            * TARGET_CHANNELS as u32
            * TARGET_BITS_PER_SAMPLE as u32
            / 8) as u64;
        let target_data_bytes = byte_rate * SECONDS as u64;

        // Load the real smoke-test PCM as the tile source.
        // Source path is the user-data copy of the smoke recording, which lives
        // outside the source tree at ~/Documents/TableScribe/... (see CLAUDE.md).
        let smoke_path = dirs::home_dir()
            .expect("home dir resolvable")
            .join("Documents/TableScribe/Campaigns/Test/audio/chunk1-smoke.wav");
        let mut smoke = Vec::new();
        File::open(&smoke_path)
            .expect("smoke WAV present at ~/Documents/TableScribe/Campaigns/Test/audio/chunk1-smoke.wav")
            .read_to_end(&mut smoke)
            .unwrap();
        assert_eq!(&smoke[0..4], b"RIFF");
        let tile_pcm: Vec<u8> = smoke[44..].to_vec();
        assert!(!tile_pcm.is_empty());

        let path = temp_path("sixty-min");
        let mut rec =
            ActiveRecording::create(path.clone(), "c".into(), "session-60m".into()).unwrap();

        let mut written: u64 = 0;
        let mut tile_cursor: usize = 0;
        // Track the final CHUNK_BYTES of source bytes we asked the recorder
        // to write, so we can confirm the BufWriter actually flushed them on
        // finalize rather than dropping the post-last-flush tail.
        let mut last_bytes = vec![0u8; CHUNK_BYTES];

        while written < target_data_bytes {
            let remaining = (target_data_bytes - written) as usize;
            let want = CHUNK_BYTES.min(remaining);
            let mut buf = Vec::with_capacity(want);
            while buf.len() < want {
                let take = (tile_pcm.len() - tile_cursor).min(want - buf.len());
                buf.extend_from_slice(&tile_pcm[tile_cursor..tile_cursor + take]);
                tile_cursor = (tile_cursor + take) % tile_pcm.len();
            }
            if want == CHUNK_BYTES {
                last_bytes.copy_from_slice(&buf);
            } else {
                last_bytes = buf.clone();
            }
            rec.append(&buf).unwrap();
            written += want as u64;
        }
        assert_eq!(written, target_data_bytes);

        let (out_path, _, _) = rec.finalize().unwrap();
        assert_eq!(out_path, path);

        let on_disk = std::fs::metadata(&path).unwrap().len();
        assert_eq!(on_disk, WAV_HEADER_LEN + target_data_bytes);

        let mut bytes = Vec::with_capacity(on_disk as usize);
        File::open(&path).unwrap().read_to_end(&mut bytes).unwrap();
        std::fs::remove_file(&path).ok();

        let parsed = parse_wav(&bytes);
        assert_eq!(parsed.audio_format, 1);
        assert_eq!(parsed.channels, TARGET_CHANNELS);
        assert_eq!(parsed.sample_rate, TARGET_SAMPLE_RATE);
        assert_eq!(parsed.bits_per_sample, TARGET_BITS_PER_SAMPLE);
        assert_eq!(parsed.fmt_size, 16);
        assert_eq!(parsed.data_size as u64, target_data_bytes);
        assert_eq!(parsed.riff_size as u64, RIFF_OVERHEAD + target_data_bytes);

        let duration_secs = parsed.data_size as f64 / byte_rate as f64;
        assert!(
            (duration_secs - SECONDS as f64).abs() < 0.001,
            "duration {} should be ~{} s",
            duration_secs,
            SECONDS
        );

        let tail = &parsed.data[parsed.data.len() - last_bytes.len()..];
        assert_eq!(tail, &last_bytes[..], "BufWriter dropped post-flush tail on finalize");
    }
}
