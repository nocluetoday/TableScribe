import { Download, Wand2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "../components/Button";
import { api } from "../lib/tauri";
import type { Campaign, SessionInfo } from "../lib/types";

const template = `# Session 001 Notes

## Session Recap

## Major Events

## NPCs Encountered

## Locations Visited

## Player Decisions

## Combat Encounters

## Items and Treasure

## Open Quests

## Unresolved Clues

## Continuity Notes

## Start of Next Session`;

export function SessionNotes({ campaign, session }: { campaign?: Campaign; session?: SessionInfo }) {
  const [contents, setContents] = useState(template);
  const [message, setMessage] = useState("");

  useEffect(() => {
    if (!campaign || !session) {
      setContents(template);
      return;
    }
    void api.getSessionNotes(campaign.id, session.sessionName).then((notes) => setContents(notes || template));
  }, [campaign?.id, session?.sessionName]);

  async function save() {
    if (!campaign || !session) return;
    const path = await api.saveSessionNotes(campaign.id, session.sessionName, contents);
    setMessage(`Saved to ${path}`);
  }

  async function exportMarkdown() {
    await save();
    setMessage("Notes saved as Markdown in the campaign notes folder.");
  }

  return (
    <div className="screen transcript-screen">
      <header className="screen-header">
        <div>
          <h1>Session Notes</h1>
          <p>{session ? `${campaign?.name || "Campaign"} / ${session.sessionName}` : "Record or import a session first."}</p>
        </div>
        <div className="button-row">
          <Button icon={<Wand2 size={18} />} disabled={!session} onClick={() => setContents(template)}>Apply Template</Button>
          <Button variant="primary" icon={<Download size={18} />} disabled={!session} onClick={exportMarkdown}>Save Markdown</Button>
        </div>
      </header>
      <textarea className="transcript-viewer" value={contents} onChange={(event) => setContents(event.target.value)} aria-label="Session notes" />
      {message && <p className="status-message">{message}</p>}
    </div>
  );
}
