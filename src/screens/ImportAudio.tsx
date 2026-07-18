import { Upload } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { Button } from "../components/Button";
import { Card } from "../components/Card";
import { api } from "../lib/tauri";
import type { Campaign } from "../lib/types";

export function ImportAudio({ campaign, onImported }: { campaign?: Campaign; onImported: () => Promise<void> }) {
  const [sessionName, setSessionName] = useState("session-001");
  const [message, setMessage] = useState("");

  async function chooseFile() {
    if (!campaign) return;
    const selected = await open({
      multiple: false,
      filters: [{ name: "Audio", extensions: ["wav", "mp3", "m4a", "flac", "ogg"] }]
    });
    if (typeof selected !== "string") return;
    const session = await api.importAudio(campaign.id, selected, sessionName);
    setMessage(`Imported ${session.sessionName}`);
    await onImported();
  }

  return (
    <div className="screen">
      <header className="screen-header">
        <div>
          <h1>Import Audio</h1>
          <p>{campaign ? `Add an existing session to ${campaign.name}.` : "Create a campaign before importing audio."}</p>
        </div>
      </header>

      <Card className="drop-zone">
        <Upload size={36} />
        <h2>Choose audio file</h2>
        <p>WAV is preferred for v0.1. Imported originals are copied into the campaign audio folder.</p>
        <label className="inline-field">
          Session name
          <input value={sessionName} onChange={(event) => setSessionName(event.target.value)} />
        </label>
        <Button variant="primary" icon={<Upload size={18} />} disabled={!campaign} onClick={chooseFile}>
          Select File
        </Button>
        {message && <p className="status-message">{message}</p>}
      </Card>
    </div>
  );
}
