mod commands;
mod platform;
mod services;
mod types;

use commands::{
    campaigns::{
        create_campaign, get_session_notes, get_transcript, import_audio, list_campaigns, list_sessions,
        save_session_notes, save_transcript,
    },
    exports::export_session,
    models::{delete_model, download_model, list_models},
    recorder::{recording_append_chunk, recording_cancel, recording_start, recording_stop},
    settings::get_app_paths,
    transcription::{
        cancel_transcription, get_transcript_document, list_transcription_jobs,
        save_transcript_document, start_transcription, TranscriptionJobsState,
    },
};
use services::recording::RecordingState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .manage(RecordingState::default())
        .manage(TranscriptionJobsState::default())
        .invoke_handler(tauri::generate_handler![
            get_app_paths,
            list_campaigns,
            create_campaign,
            list_sessions,
            import_audio,
            get_session_notes,
            save_session_notes,
            get_transcript,
            save_transcript,
            list_models,
            download_model,
            delete_model,
            list_transcription_jobs,
            cancel_transcription,
            start_transcription,
            get_transcript_document,
            save_transcript_document,
            export_session,
            recording_start,
            recording_append_chunk,
            recording_stop,
            recording_cancel
        ])
        .run(tauri::generate_context!())
        .expect("error while running TableScribe");
}
