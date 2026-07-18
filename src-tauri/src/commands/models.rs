use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::{
    platform,
    types::{ModelInfo, TableScribeResult},
};

const RECOMMENDED_MODELS: [(&str, &str); 4] = [
    ("base.en", "Fast testing"),
    ("small.en", "Live draft mode"),
    ("medium.en", "Better quality"),
    ("large-v3", "High accuracy final transcript"),
];

const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadProgress {
    pub name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<u8>,
    pub status: String,
}

#[tauri::command]
pub fn list_models() -> TableScribeResult<Vec<ModelInfo>> {
    let models_dir = platform::models_dir()?;
    fs::create_dir_all(&models_dir)?;

    RECOMMENDED_MODELS
        .iter()
        .map(|(name, recommended_use)| {
            let app_data_path = models_dir.join(format!("ggml-{}.bin", name));
            let metadata = fs::metadata(&app_data_path).ok();
            Ok(ModelInfo {
                name: name.to_string(),
                recommended_use: recommended_use.to_string(),
                installed: metadata.is_some(),
                size_bytes: metadata.map(|item| item.len()),
                path: app_data_path
                    .exists()
                    .then(|| app_data_path.display().to_string()),
            })
        })
        .collect()
}

#[tauri::command]
pub async fn download_model(app: AppHandle, name: String) -> TableScribeResult<ModelInfo> {
    tauri::async_runtime::spawn_blocking(move || download_model_sync(app, &name))
        .await
        .map_err(|error| {
            crate::types::TableScribeError::Message(format!(
                "Model download task failed: {}",
                error
            ))
        })?
}

#[tauri::command]
pub fn delete_model(name: String) -> TableScribeResult<()> {
    let path = platform::models_dir()?.join(format!("ggml-{}.bin", name));
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Pick a "draft" model for the live-partial transcription worker — small
/// and fast, never the high-accuracy final-pass model. Prefer `small.en`
/// when installed (better quality), fall back to `base.en` (always fast),
/// and finally `medium.en` if neither smaller option is around. Returns
/// `None` if no suitable draft model is installed; the caller should emit
/// a `live-partial-disabled` event and skip the worker.
pub fn draft_model_path() -> Option<PathBuf> {
    const DRAFT_PREFERENCE: &[&str] = &["small.en", "base.en", "medium.en"];

    if let Ok(models_dir) = platform::models_dir() {
        for name in DRAFT_PREFERENCE {
            let candidate = models_dir.join(format!("ggml-{}.bin", name));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    #[cfg(debug_assertions)]
    {
        for name in DRAFT_PREFERENCE {
            let preferred = dev_model_path(name);
            if preferred.exists() {
                return Some(preferred);
            }
        }
    }

    None
}

pub fn default_model_path() -> Option<PathBuf> {
    // Quality-first: prefer the largest installed model. `large-v3` is the
    // highest-quality option in the supported list; `base.en` is a fallback
    // for speed. The previous order ignored `large-v3` entirely, so an
    // installed `large-v3` was silently shadowed by a worse model elsewhere
    // (e.g. a `medium.en` bundled into the dev build output).
    const PREFERENCE: &[&str] = &["large-v3", "medium.en", "small.en", "base.en"];

    if let Ok(models_dir) = platform::models_dir() {
        for name in PREFERENCE {
            let app_data_path = models_dir.join(format!("ggml-{}.bin", name));
            if app_data_path.exists() {
                return Some(app_data_path);
            }
        }
    }

    #[cfg(debug_assertions)]
    {
        for name in PREFERENCE {
            let preferred = dev_model_path(name);
            if preferred.exists() {
                return Some(preferred);
            }
        }
    }

    None
}

pub fn bundled_model_path(resource_dir: Option<PathBuf>) -> Option<PathBuf> {
    let resource_dir = resource_dir?;
    for name in ["ggml-medium.en.bin", "ggml-base.en.bin"] {
        for candidate in [
            resource_dir.join(name),
            resource_dir
                .join("vendor")
                .join("whisper.cpp")
                .join("models")
                .join(name),
            resource_dir
                .join("_up_")
                .join("vendor")
                .join("whisper.cpp")
                .join("models")
                .join(name),
        ] {
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn dev_model_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("vendor")
        .join("whisper.cpp")
        .join("models")
        .join(format!("ggml-{}.bin", name))
}

fn download_model_sync(app: AppHandle, name: &str) -> TableScribeResult<ModelInfo> {
    if !RECOMMENDED_MODELS
        .iter()
        .any(|(candidate, _)| candidate == &name)
    {
        return Err(crate::types::TableScribeError::Message(format!(
            "Unsupported model: {}",
            name
        )));
    }

    let models_dir = platform::models_dir()?;
    fs::create_dir_all(&models_dir)?;
    let path = models_dir.join(format!("ggml-{}.bin", name));
    let temp_path = path.with_extension("bin.download");
    let url = format!("{}/ggml-{}.bin", MODEL_BASE_URL, name);
    emit_progress(&app, name, 0, None, "starting");
    let mut response = reqwest::blocking::get(url).map_err(|error| {
        crate::types::TableScribeError::Message(format!(
            "Could not start model download: {}",
            error
        ))
    })?;

    if !response.status().is_success() {
        return Err(crate::types::TableScribeError::Message(format!(
            "Model download failed with status {}",
            response.status()
        )));
    }

    let total_bytes = response.content_length();
    let mut file = fs::File::create(&temp_path)?;
    let mut downloaded_bytes = 0u64;
    let mut buffer = [0u8; 1024 * 256];

    loop {
        let read = response.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])?;
        downloaded_bytes += read as u64;
        emit_progress(&app, name, downloaded_bytes, total_bytes, "downloading");
    }

    file.flush()?;
    fs::rename(&temp_path, &path)?;
    emit_progress(&app, name, downloaded_bytes, total_bytes, "complete");

    let size_bytes = fs::metadata(&path).ok().map(|metadata| metadata.len());
    let recommended_use = RECOMMENDED_MODELS
        .iter()
        .find(|(candidate, _)| candidate == &name)
        .map(|(_, use_case)| *use_case)
        .unwrap_or("Local transcription");

    Ok(ModelInfo {
        name: name.to_string(),
        recommended_use: recommended_use.to_string(),
        installed: true,
        size_bytes,
        path: Some(path.display().to_string()),
    })
}

fn emit_progress(
    app: &AppHandle,
    name: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    status: &str,
) {
    let percent = total_bytes
        .filter(|total| *total > 0)
        .map(|total| ((downloaded_bytes.saturating_mul(100) / total).min(100)) as u8);

    let _ = app.emit(
        "model-download-progress",
        ModelDownloadProgress {
            name: name.to_string(),
            downloaded_bytes,
            total_bytes,
            percent,
            status: status.to_string(),
        },
    );
}
