use crate::types::{AppPaths, TableScribeResult};

pub use super::common::{campaigns_dir, models_dir, normalize_filename};

pub fn app_paths() -> TableScribeResult<AppPaths> {
    super::common::app_paths_for("macos")
}

pub fn whisper_cli_sidecar_name() -> &'static str {
    "whisper-cli"
}

pub fn whisper_stream_sidecar_name() -> &'static str {
    "whisper-stream"
}
