import { formatDuration } from "../lib/format";

/** Mốc thời gian bấm được để nghe lại. */
export function TimeLink({ ms, onPlay, label }: { ms: number | null; onPlay: (ms: number) => void; label?: string }) {
  if (ms === null) return null;
  return (
    <button
      onClick={() => onPlay(ms)}
      title="Bấm để nghe lại"
      className="shrink-0 rounded px-1.5 py-0.5 font-mono text-xs text-sky-300 hover:bg-sky-300/10"
    >
      ▶ {label ?? formatDuration(ms)}
    </button>
  );
}
