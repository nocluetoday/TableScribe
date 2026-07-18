import { Card } from "../components/Card";
import { displayPath } from "../lib/paths";
import type { AppPaths } from "../lib/types";

export function Settings({ paths }: { paths: AppPaths | null }) {
  return (
    <div className="screen">
      <header className="screen-header">
        <div>
          <h1>Settings</h1>
          <p>Local defaults for transcription, storage, exports, and live draft mode.</p>
        </div>
      </header>

      <div className="settings-grid">
        <Card>
          <h2>Storage</h2>
          <p>Campaigns: {paths ? displayPath(paths.campaignsDir) : "Resolving..."}</p>
          <p>Models: {paths ? displayPath(paths.modelsDir) : "Resolving..."}</p>
        </Card>
        <Card>
          <h2>Transcription</h2>
          <label className="checkbox">
            <input type="checkbox" />
            Live Draft Transcript
          </label>
          <p>Live draft mode is experimental. Final transcripts should be generated from the saved recording.</p>
        </Card>
        <Card>
          <h2>Exports</h2>
          <label className="checkbox">
            <input type="checkbox" defaultChecked />
            Markdown
          </label>
          <label className="checkbox">
            <input type="checkbox" defaultChecked />
            TXT
          </label>
          <label className="checkbox">
            <input type="checkbox" defaultChecked />
            SRT
          </label>
        </Card>
      </div>
    </div>
  );
}
