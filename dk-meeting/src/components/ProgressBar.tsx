export function ProgressBar({ value, total, label }: { value: number; total: number; label?: string }) {
  const pct = total > 0 ? Math.min(100, Math.round((value / total) * 100)) : 0;
  return (
    <div className="w-full">
      <div className="h-1.5 overflow-hidden rounded-full bg-ink-800">
        <div
          className={`h-full rounded-full bg-white transition-[width] ${total > 0 ? "" : "w-1/3 animate-pulse"}`}
          style={total > 0 ? { width: `${pct}%` } : undefined}
        />
      </div>
      {label && <p className="mt-1 text-xs text-ink-400">{label}</p>}
    </div>
  );
}
