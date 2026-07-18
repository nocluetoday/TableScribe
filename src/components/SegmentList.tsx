import { Play } from "lucide-react";
import type { TranscriptSegment } from "../lib/types";

export function SegmentList({
  segments,
  knownSpeakers,
  onSpeakerChange,
  onSeek
}: {
  segments: TranscriptSegment[];
  knownSpeakers: string[];
  onSpeakerChange: (segmentId: string, speaker: string) => void;
  onSeek: (startSeconds: number) => void;
}) {
  if (segments.length === 0) {
    return <p className="segment-empty">No segments yet — generate the transcript to see them here.</p>;
  }

  return (
    <div className="segment-list">
      <datalist id="known-speakers">
        {knownSpeakers.map((name) => (
          <option key={name} value={name} />
        ))}
      </datalist>
      {segments.map((segment) => (
        <div className="segment-row" key={segment.id}>
          <button
            type="button"
            className="segment-timestamp"
            onClick={() => onSeek(segment.startMs / 1000)}
            title={`Seek to ${formatTimestamp(segment.startMs)}`}
          >
            <Play size={12} />
            {formatTimestamp(segment.startMs)}
          </button>
          <input
            type="text"
            list="known-speakers"
            className="segment-speaker"
            placeholder="Speaker"
            value={segment.speaker ?? ""}
            onChange={(event) => onSpeakerChange(segment.id, event.target.value)}
          />
          <p className="segment-text">{segment.text}</p>
        </div>
      ))}
    </div>
  );
}

/** Render milliseconds as `m:ss.t` (one decimal). Whisper segment timestamps
 *  are not raw audio offsets; treating them as precise is misleading. */
function formatTimestamp(ms: number): string {
  const totalSeconds = ms / 1000;
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds - minutes * 60;
  return `${minutes}:${seconds.toFixed(1).padStart(4, "0")}`;
}
