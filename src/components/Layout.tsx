import { BookOpen, FileAudio, FileText, FolderOpen, Mic, NotebookTabs, Settings, Sparkles, Upload } from "lucide-react";
import type { ReactNode } from "react";
import type { ScreenId } from "../lib/types";

const nav: Array<{ id: ScreenId; label: string; icon: ReactNode }> = [
  { id: "campaigns", label: "Campaigns", icon: <FolderOpen size={18} /> },
  { id: "record-session", label: "Record", icon: <Mic size={18} /> },
  { id: "import-audio", label: "Import", icon: <Upload size={18} /> },
  { id: "queue", label: "Queue", icon: <FileAudio size={18} /> },
  { id: "transcript-review", label: "Transcript", icon: <FileText size={18} /> },
  { id: "session-notes", label: "Notes", icon: <NotebookTabs size={18} /> },
  { id: "model-manager", label: "Models", icon: <Sparkles size={18} /> },
  { id: "settings", label: "Settings", icon: <Settings size={18} /> }
];

export function Layout({
  active,
  onNavigate,
  children
}: {
  active: ScreenId;
  onNavigate: (screen: ScreenId) => void;
  children: ReactNode;
}) {
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <BookOpen size={24} />
          <div>
            <strong>TableScribe</strong>
            <span>Local session memory</span>
          </div>
        </div>
        <nav>
          {nav.map((item) => (
            <button
              key={item.id}
              className={active === item.id ? "nav-item active" : "nav-item"}
              onClick={() => onNavigate(item.id)}
            >
              {item.icon}
              <span>{item.label}</span>
            </button>
          ))}
        </nav>
      </aside>
      <main className="content">{children}</main>
    </div>
  );
}
