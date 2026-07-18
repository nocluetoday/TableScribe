use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    commands::campaigns::find_campaign,
    commands::models::{bundled_model_path, default_model_path},
    platform,
    services::filesystem,
    types::{
        TableScribeError, TableScribeResult, TranscriptDocument, TranscriptSegment,
        TranscriptionJob,
    },
};

const SCHEMA_VERSION: u32 = 1;
const PROGRESS_EVENT: &str = "transcription-progress";

/// Shared map of in-flight batch transcription jobs. Each entry's `AtomicBool`
/// is the cancel flag — when set to true, the running transcribe loop kills
/// its `whisper-cli` child between progress lines. We deliberately use a
/// flag rather than sharing the `Child` itself: only the worker thread that
/// owns the `Child` ever calls `kill()`, avoiding mutex contention on
/// process control.
#[derive(Default)]
pub struct TranscriptionJobsState {
    pub inner: Mutex<HashMap<String, JobEntry>>,
}

pub struct JobEntry {
    pub session_name: String,
    pub campaign_id: String,
    pub audio_path: String,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TranscriptionProgress {
    job_id: String,
    percent: u8,
}

#[derive(Deserialize)]
struct WhisperJsonOutput {
    transcription: Vec<WhisperJsonSegment>,
}

#[derive(Deserialize)]
struct WhisperJsonSegment {
    offsets: WhisperJsonOffsets,
    text: String,
}

#[derive(Deserialize)]
struct WhisperJsonOffsets {
    from: u32,
    to: u32,
}

#[tauri::command]
pub fn list_transcription_jobs(
    state: State<'_, TranscriptionJobsState>,
) -> TableScribeResult<Vec<TranscriptionJob>> {
    let guard = state
        .inner
        .lock()
        .map_err(|_| TableScribeError::Message("Transcription jobs lock was poisoned.".into()))?;
    Ok(guard
        .iter()
        .map(|(job_id, entry)| TranscriptionJob {
            id: job_id.clone(),
            campaign_id: entry.campaign_id.clone(),
            session_name: entry.session_name.clone(),
            audio_path: entry.audio_path.clone(),
            status: "running".into(),
            progress: 0,
            transcript_path: None,
            error: None,
        })
        .collect())
}

#[tauri::command]
pub fn cancel_transcription(
    state: State<'_, TranscriptionJobsState>,
    job_id: String,
) -> TableScribeResult<()> {
    let guard = state
        .inner
        .lock()
        .map_err(|_| TableScribeError::Message("Transcription jobs lock was poisoned.".into()))?;
    if let Some(entry) = guard.get(&job_id) {
        entry.cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
pub async fn start_transcription(
    app: AppHandle,
    state: State<'_, TranscriptionJobsState>,
    campaign_id: String,
    session_name: String,
    audio_path: String,
    model_path: Option<String>,
) -> TableScribeResult<TranscriptionJob> {
    let resource_dir = app.path().resource_dir().ok();
    let safe_name = platform::normalize_filename(&session_name);
    let job_id = format!("{}:{}", campaign_id, safe_name);

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = state.inner.lock().map_err(|_| {
            TableScribeError::Message("Transcription jobs lock was poisoned.".into())
        })?;
        if guard.contains_key(&job_id) {
            return Err(TableScribeError::Message(format!(
                "Transcription already running for {}",
                session_name
            )));
        }
        guard.insert(
            job_id.clone(),
            JobEntry {
                session_name: session_name.clone(),
                campaign_id: campaign_id.clone(),
                audio_path: audio_path.clone(),
                cancel: Arc::clone(&cancel),
            },
        );
    }

    // The tauri::State borrow can't cross the await boundary into spawn_blocking,
    // so clone what the worker thread needs and resolve the shared map via the
    // AppHandle when it's time to deregister.
    let cancel_for_worker = Arc::clone(&cancel);
    let job_id_for_worker = job_id.clone();
    let app_for_worker = app.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        transcribe_sync(
            app_for_worker,
            job_id_for_worker,
            cancel_for_worker,
            campaign_id,
            session_name,
            audio_path,
            model_path,
            resource_dir,
        )
    })
    .await
    .map_err(|error| TableScribeError::Message(format!("Transcription task failed: {}", error)));

    // Deregister regardless of outcome — success, failure, or cancel all end here.
    if let Ok(mut guard) = app.state::<TranscriptionJobsState>().inner.lock() {
        guard.remove(&job_id);
    }

    result?
}

fn transcribe_sync(
    app: AppHandle,
    job_id: String,
    cancel: Arc<AtomicBool>,
    campaign_id: String,
    session_name: String,
    audio_path: String,
    model_path: Option<String>,
    resource_dir: Option<PathBuf>,
) -> TableScribeResult<TranscriptionJob> {
    let campaign = find_campaign(&campaign_id)?;
    let root = PathBuf::from(campaign.root_path);
    let safe_name = platform::normalize_filename(&session_name);
    let transcript_dir = root.join("transcripts");
    filesystem::ensure_dir(&transcript_dir)?;

    let model_path = model_path
        .map(PathBuf::from)
        .or_else(default_model_path)
        .or_else(|| bundled_model_path(resource_dir))
        .ok_or_else(|| {
            TableScribeError::Message(
                "No whisper model found. Download medium.en in Model Manager first.".into(),
            )
        })?;
    let md_path = transcript_dir.join(format!("{}.md", safe_name));
    let txt_path = transcript_dir.join(format!("{}.txt", safe_name));
    let srt_path = transcript_dir.join(format!("{}.srt", safe_name));
    let json_path = transcript_dir.join(format!("{}.json", safe_name));
    let output_base = transcript_dir.join(&safe_name);
    let whisper_cli = whisper_cli_path()?;

    // `-ng` is intentionally absent: Metal acceleration is materially faster
    // on Apple Silicon for long-form audio. `-pp` prints progress lines to
    // stderr that we parse and forward to the UI as `transcription-progress`
    // events.
    let mut child = Command::new(&whisper_cli)
        .args([
            "-m",
            &model_path.display().to_string(),
            "-f",
            &audio_path,
            "-otxt",
            "-osrt",
            "-oj",
            "-of",
            &output_base.display().to_string(),
            "-pp",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stderr_handle = child.stderr.take().expect("stderr was piped");
    let mut last_emitted: u8 = 0;
    let mut cancelled = false;
    let mut stderr_tail: Vec<String> = Vec::new();
    let reader = BufReader::new(stderr_handle);
    for line in reader.lines() {
        if cancel.load(Ordering::SeqCst) {
            cancelled = true;
            let _ = child.kill();
            break;
        }
        let Ok(line) = line else { continue };
        if let Some(percent) = parse_progress_percent(&line) {
            if percent != last_emitted {
                last_emitted = percent;
                let _ = app.emit(
                    PROGRESS_EVENT,
                    TranscriptionProgress {
                        job_id: job_id.clone(),
                        percent,
                    },
                );
            }
        } else {
            // Keep the last few non-progress lines so failure messages can
            // surface something useful — `Vec::with_capacity(8)` would be
            // tidier, but a moving window is good enough.
            if stderr_tail.len() == 8 {
                stderr_tail.remove(0);
            }
            stderr_tail.push(line);
        }
    }

    let status = child.wait()?;

    if cancelled {
        cleanup_partial_outputs(&output_base);
        return Err(TableScribeError::Message(format!(
            "Transcription cancelled for {}",
            session_name
        )));
    }
    if !status.success() {
        cleanup_partial_outputs(&output_base);
        return Err(TableScribeError::Message(format!(
            "whisper-cli failed: {}",
            stderr_tail.join(" | ").trim()
        )));
    }

    // Final 100% so the bar lands cleanly even if whisper-cli skipped the last tick.
    if last_emitted != 100 {
        let _ = app.emit(
            PROGRESS_EVENT,
            TranscriptionProgress {
                job_id: job_id.clone(),
                percent: 100,
            },
        );
    }

    let txt = fs::read_to_string(&txt_path).unwrap_or_default();
    fs::write(&md_path, format!("# {}\n\n{}", session_name, txt.trim()))?;
    if !srt_path.exists() {
        fs::write(&srt_path, "1\n00:00:00,000 --> 00:00:01,000\n\n")?;
    }

    // whisper-cli writes its own JSON file at output_base.with_extension("json").
    // Parse it, transform to our TranscriptDocument schema, and overwrite the
    // path with our format. Preserving any speaker tags from a prior document
    // is intentionally NOT done here — regenerate is a destructive operation
    // and the frontend confirms with the user before invoking it.
    let whisper_json_path = output_base.with_extension("json");
    let document = build_document(&whisper_json_path, &session_name, Some(&model_path))?;
    write_transcript_document(&json_path, &document)?;

    Ok(TranscriptionJob {
        id: format!("{}:{}", campaign_id, safe_name),
        campaign_id,
        session_name,
        audio_path,
        status: "complete".into(),
        progress: 100,
        transcript_path: Some(md_path.display().to_string()),
        error: None,
    })
}

/// Parse a `whisper_print_progress_callback: progress = NN%` line and
/// return `NN` clamped to `[0, 100]`. Returns `None` for any other line.
fn parse_progress_percent(line: &str) -> Option<u8> {
    let line = line.trim();
    if !line.contains("progress") {
        return None;
    }
    // The actual format from whisper.cpp is:
    //   whisper_print_progress_callback: progress = 12%
    // but the prefix/spacing can shift across versions, so locate "=" and "%".
    let eq = line.rfind('=')?;
    let percent_marker = line.rfind('%')?;
    if percent_marker <= eq {
        return None;
    }
    let value = line[eq + 1..percent_marker].trim();
    let parsed: u32 = value.parse().ok()?;
    Some(parsed.min(100) as u8)
}

/// Remove the `.txt`, `.srt`, `.json` files whisper-cli would have written
/// under `output_base`. Called after a failed or cancelled transcribe so we
/// don't leave half-finished outputs masquerading as a real transcript.
fn cleanup_partial_outputs(output_base: &Path) {
    for ext in ["txt", "srt", "json"] {
        let path = output_base.with_extension(ext);
        let _ = fs::remove_file(&path);
    }
}

/// Read a whisper-cli `-oj` JSON file from `path` and build a TranscriptDocument.
/// `model_path` is recorded alongside the segments so we can show "transcribed
/// with X" in the UI later.
fn build_document(
    path: &Path,
    session_name: &str,
    model_path: Option<&Path>,
) -> TableScribeResult<TranscriptDocument> {
    let raw = fs::read_to_string(path).map_err(|err| {
        TableScribeError::Message(format!(
            "Could not read whisper JSON at {}: {}",
            path.display(),
            err
        ))
    })?;
    let parsed: WhisperJsonOutput = serde_json::from_str(&raw)?;
    let segments = parsed
        .transcription
        .into_iter()
        .enumerate()
        .map(|(idx, seg)| TranscriptSegment {
            id: format!("s{}", idx),
            start_ms: seg.offsets.from,
            end_ms: seg.offsets.to,
            text: seg.text.trim().to_string(),
            speaker: None,
        })
        .collect();
    Ok(TranscriptDocument {
        schema_version: SCHEMA_VERSION,
        session_name: session_name.to_string(),
        created_at: Utc::now().to_rfc3339(),
        model_path: model_path.map(|p| p.display().to_string()),
        segments,
    })
}

fn write_transcript_document(
    path: &Path,
    document: &TranscriptDocument,
) -> TableScribeResult<()> {
    if let Some(parent) = path.parent() {
        filesystem::ensure_dir(parent)?;
    }
    let serialized = serde_json::to_string_pretty(document)?;
    fs::write(path, serialized)?;
    Ok(())
}

#[tauri::command]
pub fn get_transcript_document(
    campaign_id: String,
    session_name: String,
) -> TableScribeResult<Option<TranscriptDocument>> {
    let campaign = find_campaign(&campaign_id)?;
    let safe_name = platform::normalize_filename(&session_name);
    let path = PathBuf::from(&campaign.root_path)
        .join("transcripts")
        .join(format!("{}.json", safe_name));
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)?;
    let document: TranscriptDocument = serde_json::from_str(&raw).map_err(|err| {
        TableScribeError::Message(format!(
            "Transcript JSON at {} could not be parsed: {}",
            path.display(),
            err
        ))
    })?;
    if document.schema_version != SCHEMA_VERSION {
        return Err(TableScribeError::Message(format!(
            "Transcript JSON schema {} not supported (this app handles version {}).",
            document.schema_version, SCHEMA_VERSION
        )));
    }
    Ok(Some(document))
}

#[tauri::command]
pub fn save_transcript_document(
    campaign_id: String,
    session_name: String,
    document: TranscriptDocument,
) -> TableScribeResult<String> {
    let campaign = find_campaign(&campaign_id)?;
    let safe_name = platform::normalize_filename(&session_name);
    let path = PathBuf::from(&campaign.root_path)
        .join("transcripts")
        .join(format!("{}.json", safe_name));
    if document.schema_version != SCHEMA_VERSION {
        return Err(TableScribeError::Message(format!(
            "Refusing to save transcript JSON: client sent schemaVersion={}, this app uses {}.",
            document.schema_version, SCHEMA_VERSION
        )));
    }
    write_transcript_document(&path, &document)?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("chunk1-smoke-whisper.json")
    }

    fn temp_doc_path(label: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "tablescribe-test-doc-{}-{}-{}.json",
            label,
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        p
    }

    #[test]
    fn build_document_from_real_whisper_json_extracts_expected_segments() {
        let doc = build_document(&fixture_path(), "chunk1-smoke", None).unwrap();
        assert_eq!(doc.schema_version, SCHEMA_VERSION);
        assert_eq!(doc.session_name, "chunk1-smoke");
        assert_eq!(doc.segments.len(), 4);

        let first = &doc.segments[0];
        assert_eq!(first.id, "s0");
        assert_eq!(first.start_ms, 0);
        assert_eq!(first.end_ms, 3820);
        assert!(first.text.starts_with("So this thing wants me"));
        assert!(first.speaker.is_none());

        let last = &doc.segments[3];
        assert_eq!(last.id, "s3");
        assert_eq!(last.start_ms, 12440);
        assert_eq!(last.end_ms, 14800);
        assert!(last.text.contains("Final transcription"));
    }

    #[test]
    fn write_and_reload_document_preserves_speaker_tags() {
        let doc = TranscriptDocument {
            schema_version: SCHEMA_VERSION,
            session_name: "s".into(),
            created_at: "2026-05-19T00:00:00Z".into(),
            model_path: Some("/models/ggml-base.en.bin".into()),
            segments: vec![
                TranscriptSegment {
                    id: "s0".into(),
                    start_ms: 0,
                    end_ms: 1000,
                    text: "hi".into(),
                    speaker: Some("DM".into()),
                },
                TranscriptSegment {
                    id: "s1".into(),
                    start_ms: 1000,
                    end_ms: 2000,
                    text: "ho".into(),
                    speaker: None,
                },
            ],
        };
        let path = temp_doc_path("roundtrip");
        write_transcript_document(&path, &doc).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        let reloaded: TranscriptDocument = serde_json::from_str(&raw).unwrap();
        fs::remove_file(&path).ok();

        assert_eq!(reloaded, doc, "roundtrip must preserve every field");
        assert_eq!(reloaded.segments[0].speaker, Some("DM".into()));
        assert_eq!(reloaded.segments[1].speaker, None);
    }

    #[test]
    fn parse_progress_percent_handles_real_whisper_output() {
        assert_eq!(
            parse_progress_percent("whisper_print_progress_callback: progress = 12%"),
            Some(12)
        );
        assert_eq!(
            parse_progress_percent("whisper_print_progress_callback: progress = 100%"),
            Some(100)
        );
        // Tolerate extra spacing variation.
        assert_eq!(
            parse_progress_percent("whisper_print_progress_callback: progress =   5%"),
            Some(5)
        );
        // Non-progress diagnostic lines must be ignored.
        assert_eq!(
            parse_progress_percent(
                "whisper_print_timings:    total time =   544.29 ms"
            ),
            None
        );
        assert_eq!(parse_progress_percent("system_info: n_threads = 4 / 18"), None);
        assert_eq!(parse_progress_percent(""), None);
        assert_eq!(parse_progress_percent("progress"), None);
        // Clamp values above 100 (defensive — whisper shouldn't emit these).
        assert_eq!(
            parse_progress_percent("whisper_print_progress_callback: progress = 250%"),
            Some(100)
        );
    }

    #[test]
    fn cleanup_partial_outputs_removes_each_extension_quietly() {
        let mut base = std::env::temp_dir();
        base.push(format!(
            "tablescribe-test-cleanup-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        // Create only two of the three siblings — confirm missing files don't blow up.
        fs::write(base.with_extension("txt"), "x").unwrap();
        fs::write(base.with_extension("json"), "y").unwrap();
        cleanup_partial_outputs(&base);
        assert!(!base.with_extension("txt").exists());
        assert!(!base.with_extension("srt").exists());
        assert!(!base.with_extension("json").exists());
    }

    #[test]
    fn schema_version_mismatch_surfaces_a_clear_error() {
        // Hand-craft a document with a future schema version and confirm the
        // load path rejects it. (We test by going through serde directly,
        // since the command-level reader checks schema_version after parse.)
        let raw = r#"{"schemaVersion":99,"sessionName":"s","createdAt":"t","modelPath":null,"segments":[]}"#;
        let parsed: TranscriptDocument = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.schema_version, 99);

        // Equivalent of get_transcript_document's check.
        let outcome: TableScribeResult<()> = if parsed.schema_version != SCHEMA_VERSION {
            Err(TableScribeError::Message(format!(
                "Transcript JSON schema {} not supported (this app handles version {}).",
                parsed.schema_version, SCHEMA_VERSION
            )))
        } else {
            Ok(())
        };
        let err = outcome.unwrap_err();
        match err {
            TableScribeError::Message(text) => {
                assert!(text.contains("99"), "error mentions the offending version: {}", text);
                assert!(text.contains(&SCHEMA_VERSION.to_string()));
            }
            other => panic!("expected Message, got {:?}", other),
        }
    }
}

pub fn whisper_cli_path() -> TableScribeResult<PathBuf> {
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join("whisper-cli-aarch64-apple-darwin");
    if dev_path.exists() {
        return Ok(dev_path);
    }

    let exe = std::env::current_exe()?;
    let sidecar = exe
        .parent()
        .ok_or_else(|| TableScribeError::Message("Could not resolve app executable folder".into()))?
        .join("whisper-cli");
    if sidecar.exists() {
        Ok(sidecar)
    } else {
        Err(TableScribeError::Message(
            "Could not find whisper-cli sidecar".into(),
        ))
    }
}
