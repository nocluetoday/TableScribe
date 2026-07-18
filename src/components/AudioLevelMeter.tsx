const bars = Array.from({ length: 12 }, (_, index) => index);

export function AudioLevelMeter({ value = 0, active = false }: { value?: number; active?: boolean }) {
  return (
    <div className="meter" aria-label="Audio input level">
      {bars.map((bar) => (
        <span
          key={bar}
          className={active && value >= (bar + 1) * 8 ? "meter-on" : ""}
          style={{ height: `${Math.max(12, Math.min(100, value + bar * 2))}%` }}
        />
      ))}
    </div>
  );
}
