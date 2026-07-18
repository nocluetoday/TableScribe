import { listen } from "@tauri-apps/api/event";
import { Play, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "../components/Button";
import { Card } from "../components/Card";
import { ProgressBar } from "../components/ProgressBar";
import { api } from "../lib/tauri";
import type { SessionInfo, TranscriptionJob } from "../lib/types";

type ProgressPayload = { jobId: string; percent: number };

const fallbackJobs: TranscriptionJob[] = [
  {
    id: "sample",
    campaignId: "demo",
    sessionName: "session-001",
    audioPath: "audio/session-001.wav",
    status: "queued",
    progress: 0
  }
];

function jobIdFor(campaignId: string, sessionName: string) {
  return `${campaignId}:${sessionName}`;
}

export function TranscriptionQueue({
  jobs,
  sessions,
  onSelectSession,
  onRefresh
}: {
  jobs: TranscriptionJob[];
  sessions: SessionInfo[];
  onSelectSession: (sessionName: string) => void;
  onRefresh: () => Promise<void>;
}) {
  const [message, setMessage] = useState("");
  const [runningJobId, setRunningJobId] = useState<string | null>(null);
  const [progress, setProgress] = useState<Record<string, number>>({});
  const visibleJobs = jobs.length ? jobs : fallbackJobs;

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void listen<ProgressPayload>("transcription-progress", (event) => {
      if (cancelled) return;
      setProgress((prev) => ({ ...prev, [event.payload.jobId]: event.payload.percent }));
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  async function transcribe(session: SessionInfo) {
    const jobId = jobIdFor(session.campaignId, session.sessionName);
    setRunningJobId(jobId);
    setProgress((prev) => ({ ...prev, [jobId]: 0 }));
    setMessage(`Transcribing ${session.sessionName}...`);
    try {
      await api.startTranscription(session.campaignId, session.sessionName, session.audioPath);
      setMessage(`Transcript files created for ${session.sessionName}`);
      onSelectSession(session.sessionName);
      await onRefresh();
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setRunningJobId(null);
      // Leave the progress entry intact so the bar stays where it landed
      // (100% on success, intermediate on cancel) until the next job starts.
    }
  }

  async function cancel(session: SessionInfo) {
    const jobId = jobIdFor(session.campaignId, session.sessionName);
    try {
      await api.cancelTranscription(jobId);
      setMessage(`Cancelling ${session.sessionName}…`);
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  return (
    <div className="screen">
      <header className="screen-header">
        <div>
          <h1>Transcription Queue</h1>
          <p>Run local whisper.cpp jobs and keep progress visible.</p>
        </div>
      </header>

      {sessions.map((session) => {
        const jobId = jobIdFor(session.campaignId, session.sessionName);
        const isRunning = runningJobId === jobId;
        const percent = progress[jobId];
        const barValue = isRunning ? (percent ?? 0) : session.transcriptMdPath ? 100 : 0;
        return (
          <Card key={session.audioPath} className="job-row">
            <div>
              <h2>{session.sessionName}</h2>
              <span>{session.audioPath}</span>
            </div>
            <ProgressBar value={barValue} />
            {isRunning ? (
              <Button icon={<X size={18} />} onClick={() => cancel(session)}>
                Cancel
              </Button>
            ) : (
              <Button
                icon={<Play size={18} />}
                disabled={runningJobId !== null}
                onClick={() => transcribe(session)}
              >
                {session.transcriptMdPath ? "Regenerate" : "Transcribe"}
              </Button>
            )}
          </Card>
        );
      })}

      {!sessions.length && visibleJobs.map((job) => (
        <Card key={job.id} className="job-row">
          <div>
            <h2>{job.sessionName}</h2>
            <span>{job.audioPath}</span>
          </div>
          <ProgressBar value={job.progress} />
          <Button icon={<Play size={18} />}>Transcribe</Button>
        </Card>
      ))}
      {message && <p className="status-message">{message}</p>}
    </div>
  );
}

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return `Transcription failed: ${JSON.stringify(error)}`;
}
