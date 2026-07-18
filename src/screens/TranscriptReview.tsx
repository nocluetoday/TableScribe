import { convertFileSrc } from "@tauri-apps/api/core";
import { Download, Save } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Button } from "../components/Button";
import { SegmentList } from "../components/SegmentList";
import { TranscriptViewer } from "../components/TranscriptViewer";
import { api } from "../lib/tauri";
import type { Campaign, SessionInfo, TranscriptDocument } from "../lib/types";

type SaveState =
  | { kind: "idle" }
  | { kind: "dirty" }
  | { kind: "saving" }
  | { kind: "saved"; at: Date }
  | { kind: "error"; message: string };

const AUTOSAVE_DELAY_MS = 1500;

export function TranscriptReview({
  campaign,
  session,
  onRefresh
}: {
  campaign?: Campaign;
  session?: SessionInfo;
  onRefresh: () => Promise<void>;
}) {
  const [message, setMessage] = useState("");
  const [selectedFormat, setSelectedFormat] = useState<"md" | "txt" | "srt" | "json">("md");
  const [contents, setContents] = useState("");
  const [document, setDocument] = useState<TranscriptDocument | null>(null);
  const [saveState, setSaveState] = useState<SaveState>({ kind: "idle" });
  const [isWorking, setIsWorking] = useState(false);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const saveTimerRef = useRef<number | null>(null);

  // Identity of the currently-loaded session, to avoid auto-saving stale state
  // after the user navigates to a different session.
  const sessionKey = campaign && session ? `${campaign.id}::${session.sessionName}` : "";
  const sessionKeyRef = useRef(sessionKey);

  useEffect(() => {
    sessionKeyRef.current = sessionKey;
    setMessage("");
    setSaveState({ kind: "idle" });
    if (!campaign || !session) {
      setContents("Record or import a session, then generate a transcript.");
      setDocument(null);
      return;
    }
    void api.getTranscriptDocument(campaign.id, session.sessionName).then((doc) => {
      if (sessionKeyRef.current !== sessionKey) return; // navigated away mid-fetch
      setDocument(doc);
    });
    void api.getTranscript(campaign.id, session.sessionName).then((md) => {
      if (sessionKeyRef.current !== sessionKey) return;
      setContents(md);
    });
  }, [campaign?.id, session?.sessionName]);

  // Debounced auto-save: any time `document` is marked dirty, fire a save
  // after AUTOSAVE_DELAY_MS of inactivity.
  useEffect(() => {
    if (saveState.kind !== "dirty") return;
    if (!campaign || !session || !document) return;
    const targetSessionKey = sessionKey;
    if (saveTimerRef.current !== null) window.clearTimeout(saveTimerRef.current);
    saveTimerRef.current = window.setTimeout(async () => {
      setSaveState({ kind: "saving" });
      try {
        await api.saveTranscriptDocument(campaign.id, session.sessionName, document);
        if (sessionKeyRef.current !== targetSessionKey) return;
        setSaveState({ kind: "saved", at: new Date() });
      } catch (error) {
        setSaveState({ kind: "error", message: errorMessage(error) });
      }
    }, AUTOSAVE_DELAY_MS);
    return () => {
      if (saveTimerRef.current !== null) window.clearTimeout(saveTimerRef.current);
    };
  }, [saveState.kind, document, campaign?.id, session?.sessionName]);

  const audioSrc = useMemo(() => {
    if (!session?.audioPath) return undefined;
    return convertFileSrc(session.audioPath);
  }, [session?.audioPath]);

  const knownSpeakers = useMemo(() => {
    if (!document) return [];
    const set = new Set<string>();
    for (const segment of document.segments) {
      if (segment.speaker && segment.speaker.trim().length > 0) set.add(segment.speaker.trim());
    }
    return Array.from(set).sort();
  }, [document]);

  async function transcribe() {
    if (!campaign || !session) return;
    if (document && document.segments.length > 0) {
      const confirmed = window.confirm(
        "Regenerating will overwrite the current transcript and any speaker tags you have set. Continue?"
      );
      if (!confirmed) return;
    }
    setIsWorking(true);
    setMessage(`Transcribing ${session.sessionName}...`);
    try {
      await api.startTranscription(campaign.id, session.sessionName, session.audioPath);
      setMessage(`Transcript generated for ${session.sessionName}`);
      const [newDoc, newMd] = await Promise.all([
        api.getTranscriptDocument(campaign.id, session.sessionName),
        api.getTranscript(campaign.id, session.sessionName)
      ]);
      setDocument(newDoc);
      setContents(newMd);
      setSaveState({ kind: "idle" });
      await onRefresh();
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setIsWorking(false);
    }
  }

  async function saveMarkdown() {
    if (!campaign || !session) return;
    try {
      const path = await api.saveTranscript(campaign.id, session.sessionName, contents);
      setMessage(`Saved markdown to ${path}`);
      await onRefresh();
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function flushDocumentSave() {
    if (!campaign || !session || !document) return;
    if (saveTimerRef.current !== null) {
      window.clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }
    setSaveState({ kind: "saving" });
    try {
      await api.saveTranscriptDocument(campaign.id, session.sessionName, document);
      setSaveState({ kind: "saved", at: new Date() });
    } catch (error) {
      setSaveState({ kind: "error", message: errorMessage(error) });
    }
  }

  async function exportFile() {
    if (!campaign || !session) return;
    try {
      const path = await api.exportSession(campaign.id, session.sessionName, selectedFormat);
      setMessage(`Exported to ${path}`);
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  function handleSpeakerChange(segmentId: string, speaker: string) {
    setDocument((prev) => {
      if (!prev) return prev;
      const trimmed = speaker.trim();
      const next: TranscriptDocument = {
        ...prev,
        segments: prev.segments.map((seg) =>
          seg.id === segmentId ? { ...seg, speaker: trimmed.length === 0 ? null : speaker } : seg
        )
      };
      return next;
    });
    setSaveState({ kind: "dirty" });
  }

  function handleSeek(startSeconds: number) {
    const audio = audioRef.current;
    if (!audio) return;
    audio.currentTime = startSeconds;
    void audio.play().catch(() => undefined);
  }

  const hasSegments = document && document.segments.length > 0;

  return (
    <div className="screen transcript-screen">
      <header className="screen-header">
        <div>
          <h1>Transcript Review</h1>
          <p>{session ? `${campaign?.name || "Campaign"} / ${session.sessionName}` : "Record or import a session first."}</p>
        </div>
        <div className="button-row">
          <select value={selectedFormat} onChange={(event) => setSelectedFormat(event.target.value as "md" | "txt" | "srt" | "json")}>
            <option value="md">Markdown</option>
            <option value="txt">TXT</option>
            <option value="srt">SRT</option>
            <option value="json">JSON</option>
          </select>
          {hasSegments ? (
            <Button icon={<Save size={18} />} disabled={!session || isWorking || saveState.kind === "saving"} onClick={flushDocumentSave}>
              {saveState.kind === "saving" ? "Saving" : "Save"}
            </Button>
          ) : (
            <Button icon={<Save size={18} />} disabled={!session || isWorking} onClick={saveMarkdown}>Save</Button>
          )}
          <Button icon={<Save size={18} />} disabled={!session || isWorking} onClick={transcribe}>
            {isWorking ? "Working" : hasSegments ? "Regenerate" : "Generate"}
          </Button>
          <Button variant="primary" icon={<Download size={18} />} disabled={!session || isWorking} onClick={exportFile}>Export</Button>
          <span className={`transcript-save-state ${saveStateClass(saveState)}`}>{saveStateLabel(saveState)}</span>
        </div>
      </header>
      {audioSrc && (
        <audio ref={audioRef} className="transcript-audio" controls src={audioSrc} preload="metadata" />
      )}
      {hasSegments ? (
        <SegmentList
          segments={document!.segments}
          knownSpeakers={knownSpeakers}
          onSpeakerChange={handleSpeakerChange}
          onSeek={handleSeek}
        />
      ) : (
        <TranscriptViewer value={contents} onChange={setContents} />
      )}
      {message && <p className="status-message">{message}</p>}
    </div>
  );
}

function saveStateLabel(state: SaveState): string {
  switch (state.kind) {
    case "idle":
      return "";
    case "dirty":
      return "Unsaved changes";
    case "saving":
      return "Saving…";
    case "saved":
      return `Saved ${formatClockTime(state.at)}`;
    case "error":
      return `Save failed: ${state.message}`;
  }
}

function saveStateClass(state: SaveState): string {
  if (state.kind === "saving") return "saving";
  if (state.kind === "error") return "error";
  return "";
}

function formatClockTime(d: Date): string {
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return `Action failed: ${JSON.stringify(error)}`;
}
