use tauri::{AppHandle, Manager};

use crate::platform;

pub fn whisper_cli_sidecar_name() -> &'static str {
    platform::whisper_cli_sidecar_name()
}

pub fn whisper_stream_sidecar_name() -> &'static str {
    platform::whisper_stream_sidecar_name()
}

pub fn app_resource_dir(app: &AppHandle) -> Option<String> {
    app.path()
        .resource_dir()
        .ok()
        .map(|path| path.display().to_string())
}
