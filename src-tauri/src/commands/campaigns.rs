use std::{fs, path::{Path, PathBuf}};

use chrono::Utc;
use uuid::Uuid;

use crate::{
    platform,
    services::{filesystem, templates},
    types::{Campaign, SessionInfo, TableScribeError, TableScribeResult},
};

#[tauri::command]
pub fn list_campaigns() -> TableScribeResult<Vec<Campaign>> {
    let root = platform::campaigns_dir()?;
    filesystem::ensure_dir(&root)?;

    let mut campaigns = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let metadata_path = entry.path().join("campaign.json");
        if metadata_path.exists() {
            let content = fs::read_to_string(metadata_path)?;
            campaigns.push(serde_json::from_str::<Campaign>(&content)?);
        }
    }

    campaigns.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(campaigns)
}

#[tauri::command]
pub fn create_campaign(name: String, system: Option<String>, description: Option<String>) -> TableScribeResult<Campaign> {
    let safe_name = platform::normalize_filename(&name);
    let root = platform::campaigns_dir()?.join(safe_name);
    create_campaign_structure(&root)?;

    let campaign = Campaign {
        id: Uuid::new_v4().to_string(),
        name,
        system,
        description,
        created_at: Utc::now().to_rfc3339(),
        root_path: root.display().to_string(),
    };

    fs::write(root.join("campaign.json"), serde_json::to_string_pretty(&campaign)?)?;
    Ok(campaign)
}

fn create_campaign_structure(root: &PathBuf) -> TableScribeResult<()> {
    for child in ["audio", "transcripts", "notes", "exports"] {
        filesystem::ensure_dir(&root.join(child))?;
    }

    filesystem::write_if_missing(&root.join("glossary.md"), "# Glossary\n")?;
    filesystem::write_if_missing(&root.join("npcs.md"), templates::NPC_INDEX)?;
    filesystem::write_if_missing(&root.join("locations.md"), templates::LOCATION_INDEX)?;
    filesystem::write_if_missing(&root.join("quests.md"), templates::QUEST_LOG)?;
    filesystem::write_if_missing(&root.join("summary.md"), templates::CAMPAIGN_SUMMARY)?;
    Ok(())
}

#[tauri::command]
pub fn list_sessions(campaign_id: String) -> TableScribeResult<Vec<SessionInfo>> {
    let campaign = find_campaign(&campaign_id)?;
    let root = PathBuf::from(&campaign.root_path);
    let audio_dir = root.join("audio");
    filesystem::ensure_dir(&audio_dir)?;

    let mut sessions = Vec::new();
    for entry in fs::read_dir(audio_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        sessions.push(session_from_name(&campaign.id, &root, stem, &path)?);
    }

    sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(sessions)
}

#[tauri::command]
pub fn import_audio(campaign_id: String, source_path: String, session_name: String) -> TableScribeResult<SessionInfo> {
    let campaign = find_campaign(&campaign_id)?;
    let root = PathBuf::from(&campaign.root_path);
    let safe_name = platform::normalize_filename(&session_name);
    let destination = filesystem::copy_into_dir(Path::new(&source_path), &root.join("audio"), &safe_name)?;
    let notes_path = root.join("notes").join(format!("{}-notes.md", safe_name));
    filesystem::write_if_missing(&notes_path, templates::SESSION_NOTES)?;
    session_from_name(&campaign.id, &root, &safe_name, &destination)
}

#[tauri::command]
pub fn get_session_notes(campaign_id: String, session_name: String) -> TableScribeResult<String> {
    let campaign = find_campaign(&campaign_id)?;
    let safe_name = platform::normalize_filename(&session_name);
    let path = PathBuf::from(campaign.root_path).join("notes").join(format!("{}-notes.md", safe_name));
    if !path.exists() {
        filesystem::write_if_missing(&path, templates::SESSION_NOTES)?;
    }
    Ok(fs::read_to_string(path)?)
}

#[tauri::command]
pub fn save_session_notes(campaign_id: String, session_name: String, contents: String) -> TableScribeResult<String> {
    let campaign = find_campaign(&campaign_id)?;
    let safe_name = platform::normalize_filename(&session_name);
    let path = PathBuf::from(campaign.root_path).join("notes").join(format!("{}-notes.md", safe_name));
    if let Some(parent) = path.parent() {
        filesystem::ensure_dir(parent)?;
    }
    fs::write(&path, contents)?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn get_transcript(campaign_id: String, session_name: String) -> TableScribeResult<String> {
    let campaign = find_campaign(&campaign_id)?;
    let safe_name = platform::normalize_filename(&session_name);
    let path = PathBuf::from(campaign.root_path).join("transcripts").join(format!("{}.md", safe_name));
    if path.exists() {
        Ok(fs::read_to_string(path)?)
    } else {
        Ok(format!("# {}\n\nNo transcript has been generated yet.\n", session_name))
    }
}

#[tauri::command]
pub fn save_transcript(campaign_id: String, session_name: String, contents: String) -> TableScribeResult<String> {
    let campaign = find_campaign(&campaign_id)?;
    let safe_name = platform::normalize_filename(&session_name);
    let path = PathBuf::from(campaign.root_path).join("transcripts").join(format!("{}.md", safe_name));
    if let Some(parent) = path.parent() {
        filesystem::ensure_dir(parent)?;
    }
    fs::write(&path, contents)?;
    Ok(path.display().to_string())
}

pub fn find_campaign(campaign_id: &str) -> TableScribeResult<Campaign> {
    list_campaigns()?
        .into_iter()
        .find(|campaign| campaign.id == campaign_id)
        .ok_or_else(|| TableScribeError::Message("Campaign not found".into()))
}

pub fn session_from_name(campaign_id: &str, root: &Path, session_name: &str, audio_path: &Path) -> TableScribeResult<SessionInfo> {
    let transcript_md = root.join("transcripts").join(format!("{}.md", session_name));
    let transcript_txt = root.join("transcripts").join(format!("{}.txt", session_name));
    let transcript_srt = root.join("transcripts").join(format!("{}.srt", session_name));
    let notes = root.join("notes").join(format!("{}-notes.md", session_name));
    let created_at = audio_path
        .metadata()
        .and_then(|metadata| metadata.created().or_else(|_| metadata.modified()))
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| chrono::DateTime::<chrono::Utc>::from(std::time::UNIX_EPOCH + duration).to_rfc3339())
        .unwrap_or_else(|| Utc::now().to_rfc3339());

    Ok(SessionInfo {
        campaign_id: campaign_id.to_string(),
        session_name: session_name.to_string(),
        created_at,
        audio_path: audio_path.display().to_string(),
        transcript_md_path: transcript_md.exists().then(|| transcript_md.display().to_string()),
        transcript_txt_path: transcript_txt.exists().then(|| transcript_txt.display().to_string()),
        transcript_srt_path: transcript_srt.exists().then(|| transcript_srt.display().to_string()),
        notes_path: notes.exists().then(|| notes.display().to_string()),
    })
}
