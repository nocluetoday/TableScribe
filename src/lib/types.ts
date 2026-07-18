export type SupportedPlatform = "macos" | "windows";

export type ScreenId =
  | "campaigns"
  | "new-campaign"
  | "record-session"
  | "import-audio"
  | "queue"
  | "transcript-review"
  | "session-notes"
  | "model-manager"
  | "settings";

export type Campaign = {
  id: string;
  name: string;
  system?: string;
  description?: string;
  createdAt: string;
  rootPath: string;
};

export type AppPaths = {
  platform: SupportedPlatform;
  documentsDir: string;
  appDataDir: string;
  campaignsDir: string;
  modelsDir: string;
};

export type ModelInfo = {
  name: string;
  recommendedUse: string;
  installed: boolean;
  sizeBytes?: number;
  path?: string;
};

export type TranscriptionJob = {
  id: string;
  campaignId: string;
  sessionName: string;
  audioPath: string;
  status: "queued" | "running" | "complete" | "failed";
  progress: number;
  transcriptPath?: string;
  error?: string;
};

export type SessionInfo = {
  campaignId: string;
  sessionName: string;
  createdAt: string;
  audioPath: string;
  transcriptMdPath?: string;
  transcriptTxtPath?: string;
  transcriptSrtPath?: string;
  notesPath?: string;
};

export type TranscriptSegment = {
  id: string;
  startMs: number;
  endMs: number;
  text: string;
  /** null/undefined means untagged. Free text in v1 — renames are manual. */
  speaker: string | null;
};

export type TranscriptDocument = {
  schemaVersion: number;
  sessionName: string;
  createdAt: string;
  modelPath?: string | null;
  segments: TranscriptSegment[];
};
