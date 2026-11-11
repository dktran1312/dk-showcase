import { TRACK_LABEL, type Track } from "../lib/types";

/** Đồng hồ mức âm ngang: thanh = RMS, vạch dọc = peak. Thang 0..1 tương ứng -60..0 dBFS. */
export function LevelMeter({ track, rms, peak, active }: { track: Track; rms: number; peak: number; active: boolean }) {
  const hot = peak > 0.95;
  return (
    <div className="flex items-center gap-3">
      <span className="w-24 shrink-0 text-xs font-medium text-ink-300">{TRACK_LABEL[track]}</span>
      <div className="relative h-2 flex-1 overflow-hidden rounded-full bg-ink-800">
        <div
          className={`absolute inset-y-0 left-0 rounded-full transition-[width] duration-75 ${
            active ? (hot ? "bg-rec" : rms > 0.75 ? "bg-warn" : "bg-ok") : "bg-ink-600"
          }`}
          style={{ width: `${Math.round(rms * 100)}%` }}
        />
        <div
          className="absolute inset-y-0 w-0.5 bg-white/80 transition-[left] duration-75"
          style={{ left: `calc(${Math.round(peak * 100)}% - 1px)`, opacity: peak > 0.02 ? 1 : 0 }}
        />
      </div>
    </div>
  );
}
