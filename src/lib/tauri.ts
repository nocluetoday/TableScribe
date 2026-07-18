import { invoke } from "@tauri-apps/api/core";
import type {
  AppPaths,
  Campaign,
  ModelInfo,
  SessionInfo,
  TranscriptDocument,
  TranscriptionJob
} from "./types";

const demoCampaigns: Campaign[] = [
  {
    id: "demo",
    name: "Curse of Strahd",
    system: "Dungeons & Dragons",
    description: "Local sample campaign structure",
    createdAt: new Date().toISOString(),
    rootPath: "~/Documents/TableScribe/Campaigns/Curse of Strahd"
  }
];

const demoModels: ModelInfo[] = [
  { name: "base.en", recommendedUse: "Fast testing", installed: false },
  { name: "small.en", recommendedUse: "Live draft mode", installed: false },
  { name: "medium.en", recommendedUse: "Better quality", installed: false },
  { name: "large-v3", recommendedUse: "High accuracy final transcript", installed: false }
];

const fallbackPaths: AppPaths = {
  platform: navigator.platform.toLowerCase().includes("win") ? "windows" : "macos",
  documentsDir: "~/Documents",
  appDataDir: "~/Library/Application Support/TableScribe",
  campaignsDir: "~/Documents/TableScribe/Campaigns",
  modelsDir: "~/Library/Application Support/TableScribe/models"
};

async function safeInvoke<T>(command: string, args?: Record<string, unknown>, fallback?: T): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (fallback !== undefined) {
      console.info(`Using browser fallback for ${command}`, error);
      return fallback;
    }
    throw error;
  }
}

export const api = {
  getPaths: () => safeInvoke<AppPaths>("get_app_paths", undefined, fallbackPaths),
  listCampaigns: () => safeInvoke<Campaign[]>("list_campaigns", undefined, demoCampaigns),
  createCampaign: (name: string, system?: string, description?: string) =>
    safeInvoke<Campaign>("create_campaign", { name, system, description }),
  listSessions: (campaignId: string) => safeInvoke<SessionInfo[]>("list_sessions", { campaignId }, []),
  importAudio: (campaignId: string, sourcePath: string, sessionName: string) =>
    safeInvoke<SessionInfo>("import_audio", { campaignId, sourcePath, sessionName }),
  recordingStart: (campaignId: string, sessionName: string) =>
    safeInvoke<string>("recording_start", { campaignId, sessionName }),
  recordingAppendChunk: (chunkB64: string) =>
    safeInvoke<void>("recording_append_chunk", { chunkB64 }),
  recordingStop: () => safeInvoke<SessionInfo>("recording_stop"),
  recordingCancel: () => safeInvoke<void>("recording_cancel"),
  getSessionNotes: (campaignId: string, sessionName: string) =>
    safeInvoke<string>("get_session_notes", { campaignId, sessionName }, ""),
  saveSessionNotes: (campaignId: string, sessionName: string, contents: string) =>
    safeInvoke<string>("save_session_notes", { campaignId, sessionName, contents }),
  getTranscript: (campaignId: string, sessionName: string) =>
    safeInvoke<string>("get_transcript", { campaignId, sessionName }, ""),
  saveTranscript: (campaignId: string, sessionName: string, contents: string) =>
    safeInvoke<string>("save_transcript", { campaignId, sessionName, contents }),
  listModels: () => safeInvoke<ModelInfo[]>("list_models", undefined, demoModels),
  downloadModel: (name: string) => safeInvoke<ModelInfo>("download_model", { name }),
  deleteModel: (name: string) => safeInvoke<void>("delete_model", { name }),
  listJobs: () => safeInvoke<TranscriptionJob[]>("list_transcription_jobs", undefined, []),
  startTranscription: (campaignId: string, sessionName: string, audioPath: string, modelPath?: string) =>
    safeInvoke<TranscriptionJob>("start_transcription", { campaignId, sessionName, audioPath, modelPath }),
  cancelTranscription: (jobId: string) =>
    safeInvoke<void>("cancel_transcription", { jobId }),
  getTranscriptDocument: (campaignId: string, sessionName: string) =>
    safeInvoke<TranscriptDocument | null>("get_transcript_document", { campaignId, sessionName }),
  saveTranscriptDocument: (campaignId: string, sessionName: string, document: TranscriptDocument) =>
    safeInvoke<string>("save_transcript_document", { campaignId, sessionName, document }),
  exportSession: (campaignId: string, sessionName: string, format: "md" | "txt" | "srt" | "json") =>
    safeInvoke<string>("export_session", { campaignId, sessionName, format })
};
