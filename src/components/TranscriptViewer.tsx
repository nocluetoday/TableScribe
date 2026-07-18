export function TranscriptViewer({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return (
    <textarea
      className="transcript-viewer"
      value={value}
      onChange={(event) => onChange(event.target.value)}
      aria-label="Editable transcript"
      spellCheck
    />
  );
}
