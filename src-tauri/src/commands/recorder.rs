use std::{path::{Path, PathBuf}, time::Duration};

use base64::Engine;
use tauri::{AppHandle, Emitter, State};

use crate::{
    commands::{
        campaigns::{find_campaign, session_from_name},
        models::draft_model_path,
        transcription::whisper_cli_path,
    },
    platform,
    services::{
        filesystem,
        live_transcribe::{LiveConfig, LiveDisabled, LiveTranscribeWorker, LIVE_DISABLED_EVENT},
        recording::{ActiveRecording, RecordingState},
        templates,
    },
    types::{SessionInfo, TableScribeError, TableScribeResult},
};

const LIVE_CADENCE: Duration = Duration::from_secs(8);
const LIVE_WINDOW_SECS: u32 = 12;

fn ensure_session_name_available(audio_path: &Path, safe_name: &str) -> TableScribeResult<()> {
    if audio_path.exists() {
        return Err(TableScribeError::Message(format!(
            "A recording named \"{}\" already exists in this campaign. Choose a different session name.",
            safe_name
        )));
    }
    Ok(())
}

#[tauri::command]
pub fn recording_start(
    app: AppHandle,
    state: State<'_, RecordingState>,
    campaign_id: String,
    session_name: String,
) -> TableScribeResult<String> {
    let campaign = find_campaign(&campaign_id)?;
    let root = PathBuf::from(&campaign.root_path);
    let safe_name = platform::normalize_filename(&session_name);
    let audio_path = root.join("audio").join(format!("{}.wav", safe_name));

    let mut guard = state
        .inner
        .lock()
        .map_err(|_| TableScribeError::Message("Recording lock was poisoned.".into()))?;
    if guard.is_some() {
        return Err(TableScribeError::Message(
            "A recording is already in progress. Stop or cancel it before starting another.".into(),
        ));
    }
    ensure_session_name_available(&audio_path, &safe_name)?;
    let recording = ActiveRecording::create(audio_path.clone(), campaign.id.clone(), safe_name)?;
    *guard = Some(recording);
    drop(guard);

    // Spawn the live-draft worker. If anything required is missing we emit a
    // one-shot `live-partial-disabled` event and the recording proceeds
    // without a draft transcript — never a hard failure here.
    spawn_live_worker(&app, &audio_path, &state);

    Ok(audio_path.display().to_string())
}

fn spawn_live_worker(
    app: &AppHandle,
    audio_path: &Path,
    state: &State<'_, RecordingState>,
) {
    let model_path = match draft_model_path() {
        Some(path) => path,
        None => {
            let _ = app.emit(
                LIVE_DISABLED_EVENT,
                LiveDisabled {
                    reason:
                        "No draft transcription model installed. Download base.en or small.en in Model Manager."
                            .into(),
                },
            );
            return;
        }
    };
    let whisper_cli = match whisper_cli_path() {
        Ok(path) => path,
        Err(err) => {
            let _ = app.emit(
                LIVE_DISABLED_EVENT,
                LiveDisabled {
                    reason: format!("Could not locate whisper-cli: {}", err),
                },
            );
            return;
        }
    };

    let worker = LiveTranscribeWorker::spawn(
        app.clone(),
        LiveConfig {
            audio_path: audio_path.to_path_buf(),
            model_path,
            whisper_cli,
            cadence: LIVE_CADENCE,
            window_seconds: LIVE_WINDOW_SECS,
        },
    );

    if let Ok(mut live_guard) = state.live.lock() {
        if let Some(existing) = live_guard.take() {
            existing.stop();
        }
        *live_guard = Some(worker);
    }
}

fn stop_live_worker(state: &State<'_, RecordingState>) {
    if let Ok(mut guard) = state.live.lock() {
        if let Some(worker) = guard.take() {
            worker.stop();
        }
    }
}

#[tauri::command]
pub fn recording_append_chunk(
    state: State<'_, RecordingState>,
    chunk_b64: String,
) -> TableScribeResult<()> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(chunk_b64.as_bytes())
        .map_err(|err| TableScribeError::Message(format!("Invalid chunk encoding: {}", err)))?;
    let mut guard = state
        .inner
        .lock()
        .map_err(|_| TableScribeError::Message("Recording lock was poisoned.".into()))?;
    let recording = guard
        .as_mut()
        .ok_or_else(|| TableScribeError::Message("No active recording.".into()))?;
    recording.append(&bytes)
}

#[tauri::command]
pub fn recording_stop(state: State<'_, RecordingState>) -> TableScribeResult<SessionInfo> {
    // Stop the live-draft worker first so it can't race with finalize() —
    // we don't want a tick reading the WAV after we've started rewriting
    // the RIFF/data sizes in the header.
    stop_live_worker(&state);

    let recording = {
        let mut guard = state
            .inner
            .lock()
            .map_err(|_| TableScribeError::Message("Recording lock was poisoned.".into()))?;
        guard
            .take()
            .ok_or_else(|| TableScribeError::Message("No active recording to stop.".into()))?
    };
    let (path, campaign_id, session_name) = recording.finalize()?;
    let campaign = find_campaign(&campaign_id)?;
    let root = PathBuf::from(&campaign.root_path);
    let notes_path = root.join("notes").join(format!("{}-notes.md", session_name));
    filesystem::write_if_missing(&notes_path, templates::SESSION_NOTES)?;
    session_from_name(&campaign.id, &root, &session_name, &path)
}

#[tauri::command]
pub fn recording_cancel(state: State<'_, RecordingState>) -> TableScribeResult<()> {
    stop_live_worker(&state);

    let recording = {
        let mut guard = state
            .inner
            .lock()
            .map_err(|_| TableScribeError::Message("Recording lock was poisoned.".into()))?;
        guard.take()
    };
    if let Some(recording) = recording {
        recording.discard()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_path(label: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "tablescribe-test-recorder-{}-{}-{}.wav",
            label,
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        path
    }

    #[test]
    fn ensure_session_name_available_passes_when_missing() {
        let path = temp_path("missing");
        assert!(!path.exists());
        assert!(ensure_session_name_available(&path, "session-001").is_ok());
    }

    #[test]
    fn ensure_session_name_available_rejects_existing_file_without_touching_it() {
        let path = temp_path("collision");
        let canary = b"do-not-truncate-me";
        fs::write(&path, canary).unwrap();

        let result = ensure_session_name_available(&path, "session-001");
        let preserved = fs::read(&path).unwrap();
        fs::remove_file(&path).ok();

        let err = result.expect_err("guard must reject an existing WAV");
        let msg = match err {
            TableScribeError::Message(text) => text,
            other => panic!("expected Message variant, got {:?}", other),
        };
        assert!(
            msg.contains("session-001"),
            "error should name the colliding session: {}",
            msg
        );
        assert_eq!(
            preserved, canary,
            "guard must not modify the file it rejects"
        );
    }
}
