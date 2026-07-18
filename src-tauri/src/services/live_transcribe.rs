// Live-draft transcription worker.
//
// Once per cadence, reads the trailing window from the in-progress WAV,
// wraps it in a fresh temp WAV, runs whisper-cli with a small "draft"
// model, and emits a `live-partial` Tauri event with the resulting text.
//
// Sequential by construction: a single std::thread runs ticks back-to-back
// with a sleep between them, so we cannot queue overlapping inference
// passes. If a tick ever exceeds cadence, the next one simply starts late
// — never doubled up. Inference on base.en for a 12-15 s window is
// ~0.5 s on Apple Silicon today (measured 2026-05-19), so headroom is
// ample; we'll revisit if we ever swap in a larger draft model.

use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::{
    services::recording::{TARGET_BITS_PER_SAMPLE, TARGET_CHANNELS, TARGET_SAMPLE_RATE, WAV_HEADER_LEN},
    types::{TableScribeError, TableScribeResult},
};

pub const LIVE_PARTIAL_EVENT: &str = "live-partial";
pub const LIVE_DISABLED_EVENT: &str = "live-partial-disabled";

const RIFF_OVERHEAD: u32 = 36;
const CANCEL_POLL: Duration = Duration::from_millis(200);

#[derive(Clone)]
pub struct LiveConfig {
    pub audio_path: PathBuf,
    pub model_path: PathBuf,
    pub whisper_cli: PathBuf,
    pub cadence: Duration,
    pub window_seconds: u32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LivePartial {
    text: String,
    recorded_seconds: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LiveDisabled {
    pub reason: String,
}

pub struct LiveTranscribeWorker {
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl LiveTranscribeWorker {
    pub fn spawn(app: AppHandle, config: LiveConfig) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_inner = Arc::clone(&cancel);
        let handle = thread::spawn(move || run_worker(app, config, cancel_inner));
        Self {
            cancel,
            handle: Some(handle),
        }
    }

    pub fn stop(mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for LiveTranscribeWorker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn run_worker(app: AppHandle, config: LiveConfig, cancel: Arc<AtomicBool>) {
    let byte_rate = (TARGET_SAMPLE_RATE
        * TARGET_CHANNELS as u32
        * TARGET_BITS_PER_SAMPLE as u32
        / 8) as u64;
    let window_bytes = byte_rate * config.window_seconds as u64;
    let start = Instant::now();

    while !cancel.load(Ordering::SeqCst) {
        let next_tick_at = Instant::now() + config.cadence;
        while Instant::now() < next_tick_at {
            if cancel.load(Ordering::SeqCst) {
                return;
            }
            let remaining = next_tick_at.saturating_duration_since(Instant::now());
            thread::sleep(remaining.min(CANCEL_POLL));
        }
        if cancel.load(Ordering::SeqCst) {
            return;
        }

        let recorded_seconds = start.elapsed().as_secs();

        match run_one_tick(&config, window_bytes, byte_rate) {
            Ok(Some(text)) => {
                let _ = app.emit(
                    LIVE_PARTIAL_EVENT,
                    LivePartial {
                        text,
                        recorded_seconds,
                    },
                );
            }
            Ok(None) => {
                // Not enough audio yet, or whisper produced silence/blank.
            }
            Err(err) => {
                eprintln!("[live-transcribe] tick failed: {}", err);
            }
        }
    }
}

fn run_one_tick(
    config: &LiveConfig,
    window_bytes: u64,
    byte_rate: u64,
) -> TableScribeResult<Option<String>> {
    let tmp_wav = TempFile::new("window", ".wav")?;
    match extract_trailing_window(&config.audio_path, tmp_wav.path(), window_bytes, byte_rate)? {
        Some(_) => {}
        None => return Ok(None),
    }

    // whisper-cli writes <base>.txt; track that too so Drop cleans it up.
    let tmp_base = TempFile::new("out", "")?;
    let txt_sidecar = tmp_base.path().with_extension("txt");
    let _txt_cleanup = TempFile::adopt(txt_sidecar.clone());

    let output = Command::new(&config.whisper_cli)
        .args([
            "-m",
            &config.model_path.display().to_string(),
            "-f",
            &tmp_wav.path().display().to_string(),
            "-otxt",
            "-of",
            &tmp_base.path().display().to_string(),
            "-ng",
            "-nt",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TableScribeError::Message(format!(
            "whisper-cli failed: {}",
            stderr.trim()
        )));
    }

    let text = fs::read_to_string(&txt_sidecar).unwrap_or_default();
    Ok(meaningful_text(&text))
}

fn meaningful_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Whisper conventional non-speech tokens.
    if trimmed.contains("[BLANK_AUDIO]") || trimmed.contains("(silence)") {
        return None;
    }
    // Too-short bursts are almost always hallucinations on near-silence.
    if trimmed.chars().filter(|c| c.is_alphanumeric()).count() < 3 {
        return None;
    }
    Some(trimmed.to_string())
}

/// Read the trailing `window_bytes` of raw Int16 mono LE PCM from a WAV that
/// `ActiveRecording` is currently writing (44-byte header, no other chunks)
/// and wrap that PCM in a fresh standalone WAV at `dest`. Returns the
/// number of audio bytes written, or `None` if less than 1 second of audio
/// is available yet — the worker should skip the tick in that case.
pub fn extract_trailing_window(
    source: &Path,
    dest: &Path,
    window_bytes: u64,
    byte_rate: u64,
) -> TableScribeResult<Option<u64>> {
    let total_len = fs::metadata(source)?.len();
    if total_len <= WAV_HEADER_LEN {
        return Ok(None);
    }
    let data_len = total_len - WAV_HEADER_LEN;
    let take = window_bytes.min(data_len);
    if take < byte_rate {
        return Ok(None);
    }
    let start_offset = WAV_HEADER_LEN + (data_len - take);

    let mut wav = File::open(source)?;
    wav.seek(SeekFrom::Start(start_offset))?;
    let mut pcm = vec![0u8; take as usize];
    wav.read_exact(&mut pcm)?;
    drop(wav);

    write_wav(dest, &pcm)?;
    Ok(Some(take))
}

fn write_wav(path: &Path, pcm: &[u8]) -> std::io::Result<()> {
    let data_size = pcm.len() as u32;
    let byte_rate =
        TARGET_SAMPLE_RATE * TARGET_CHANNELS as u32 * TARGET_BITS_PER_SAMPLE as u32 / 8;
    let block_align = TARGET_CHANNELS * TARGET_BITS_PER_SAMPLE / 8;
    let mut f = File::create(path)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(RIFF_OVERHEAD + data_size).to_le_bytes())?;
    f.write_all(b"WAVE")?;
    f.write_all(b"fmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&TARGET_CHANNELS.to_le_bytes())?;
    f.write_all(&TARGET_SAMPLE_RATE.to_le_bytes())?;
    f.write_all(&byte_rate.to_le_bytes())?;
    f.write_all(&block_align.to_le_bytes())?;
    f.write_all(&TARGET_BITS_PER_SAMPLE.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_size.to_le_bytes())?;
    f.write_all(pcm)?;
    Ok(())
}

/// RAII guard for a file in std::env::temp_dir(). Drops the file on drop.
/// Adopt-mode lets us also clean up sibling files that another process
/// (whisper-cli) writes alongside our base path.
struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn new(prefix: &str, suffix: &str) -> std::io::Result<Self> {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "tablescribe-live-{}-{}-{}{}",
            prefix,
            std::process::id(),
            uuid::Uuid::new_v4(),
            suffix
        ));
        Ok(Self { path })
    }

    fn adopt(path: PathBuf) -> Self {
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meaningful_text_filters_blank_and_short() {
        assert_eq!(meaningful_text(""), None);
        assert_eq!(meaningful_text("   \n  "), None);
        assert_eq!(meaningful_text("[BLANK_AUDIO]"), None);
        assert_eq!(meaningful_text(" [BLANK_AUDIO] "), None);
        assert_eq!(meaningful_text("(silence)"), None);
        assert_eq!(meaningful_text(",,"), None);
        assert_eq!(meaningful_text("a"), None); // too few alphanumeric chars
        assert_eq!(
            meaningful_text(" so this thing wants me to talk \n"),
            Some("so this thing wants me to talk".into())
        );
    }

    #[test]
    fn write_wav_roundtrips_through_a_parser() {
        let pcm: Vec<u8> = (0..(16_000 * 2 * 3)).map(|i| (i as u8).wrapping_mul(7)).collect(); // 3 s mono Int16
        let mut path = std::env::temp_dir();
        path.push(format!(
            "tablescribe-test-livewav-{}-{}.wav",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        write_wav(&path, &pcm).unwrap();
        let bytes = fs::read(&path).unwrap();
        fs::remove_file(&path).ok();

        assert_eq!(&bytes[0..4], b"RIFF");
        let riff_size = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        assert_eq!(riff_size as usize, 36 + pcm.len());
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        let sample_rate = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        assert_eq!(sample_rate, TARGET_SAMPLE_RATE);
        let channels = u16::from_le_bytes(bytes[22..24].try_into().unwrap());
        assert_eq!(channels, TARGET_CHANNELS);
        let bits = u16::from_le_bytes(bytes[34..36].try_into().unwrap());
        assert_eq!(bits, TARGET_BITS_PER_SAMPLE);
        assert_eq!(&bytes[36..40], b"data");
        let data_size = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
        assert_eq!(data_size as usize, pcm.len());
        assert_eq!(&bytes[44..], &pcm[..]);
    }

    #[test]
    fn extract_trailing_window_pulls_last_n_bytes_into_fresh_wav() {
        use crate::services::recording::ActiveRecording;

        let src = std::env::temp_dir().join(format!(
            "tablescribe-test-extract-src-{}-{}.wav",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let dest = std::env::temp_dir().join(format!(
            "tablescribe-test-extract-dest-{}-{}.wav",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));

        // Build a 5-second source via the real ActiveRecording writer so the
        // input matches what the live worker sees in production.
        let byte_rate =
            (TARGET_SAMPLE_RATE * TARGET_CHANNELS as u32 * TARGET_BITS_PER_SAMPLE as u32 / 8) as u64;
        let total_bytes = byte_rate * 5;
        let pcm: Vec<u8> = (0..total_bytes as usize)
            .map(|i| (i as u8).wrapping_mul(13))
            .collect();
        let mut rec = ActiveRecording::create(src.clone(), "c".into(), "s".into()).unwrap();
        rec.append(&pcm).unwrap();
        let _ = rec.finalize().unwrap();

        // Pull the last 2 seconds.
        let window_bytes = byte_rate * 2;
        let returned =
            extract_trailing_window(&src, &dest, window_bytes, byte_rate).unwrap();
        assert_eq!(returned, Some(window_bytes));

        let bytes = fs::read(&dest).unwrap();
        fs::remove_file(&src).ok();
        fs::remove_file(&dest).ok();

        // Header sanity.
        assert_eq!(&bytes[0..4], b"RIFF");
        let data_size = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as u64;
        assert_eq!(data_size, window_bytes);

        // PCM tail of the source must equal the dest body, byte-for-byte.
        let expected_tail = &pcm[(pcm.len() as u64 - window_bytes) as usize..];
        assert_eq!(&bytes[44..], expected_tail);
    }

    #[test]
    fn extract_trailing_window_returns_none_when_audio_too_short() {
        use crate::services::recording::ActiveRecording;

        let src = std::env::temp_dir().join(format!(
            "tablescribe-test-extract-short-{}-{}.wav",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let dest = std::env::temp_dir().join(format!(
            "tablescribe-test-extract-short-dest-{}-{}.wav",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));

        let byte_rate =
            (TARGET_SAMPLE_RATE * TARGET_CHANNELS as u32 * TARGET_BITS_PER_SAMPLE as u32 / 8) as u64;
        let mut rec = ActiveRecording::create(src.clone(), "c".into(), "s".into()).unwrap();
        // ~100 ms of audio — below the 1-second minimum.
        let pcm = vec![0u8; (byte_rate / 10) as usize];
        rec.append(&pcm).unwrap();
        let _ = rec.finalize().unwrap();

        let result = extract_trailing_window(&src, &dest, byte_rate * 12, byte_rate).unwrap();
        fs::remove_file(&src).ok();
        let dest_exists = dest.exists();
        fs::remove_file(&dest).ok();

        assert_eq!(result, None);
        assert!(!dest_exists, "extractor must not write dest when it bails out");
    }

    #[test]
    fn temp_file_deletes_on_drop() {
        let path;
        {
            let t = TempFile::new("dropme", ".wav").unwrap();
            path = t.path().to_path_buf();
            // Create it so Drop has something to delete.
            fs::write(&path, b"hi").unwrap();
            assert!(path.exists());
        }
        assert!(!path.exists(), "TempFile must remove its path on drop");
    }
}
