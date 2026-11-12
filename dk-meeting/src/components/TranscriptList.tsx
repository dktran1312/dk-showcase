import { formatDuration } from "../lib/format";
import { TRACK_LABEL, type Segment, type Track } from "../lib/types";

/** Transcript sau cuộc họp: bấm vào câu để nghe lại đúng chỗ. */
export function TranscriptList({ segments, onPlay }: { segments: Segment[]; onPlay: (ms: number, track: Track) => void }) {
  return (
    <div className="selectable divide-y divide-ink-850 rounded-xl border border-ink-800 bg-ink-900">
      {segments.map((s) => {
        const foreign = s.lang && s.lang !== "vi";
        return (
          <div key={s.id} className="group flex gap-4 px-5 py-3 hover:bg-ink-850">
            <button
              onClick={() => onPlay(s.startMs, s.track)}
              title="Bấm để nghe lại"
              className="w-14 shrink-0 pt-0.5 text-left font-mono text-xs text-sky-300 hover:underline"
            >
              {formatDuration(s.startMs)}
            </button>
            <div className="min-w-0 flex-1 cursor-pointer" onClick={() => onPlay(s.startMs, s.track)}>
              <p className="mb-0.5 text-[11px]">
                <span className={s.track === "mic" ? "text-sky-300" : "text-violet-300"}>{TRACK_LABEL[s.track]}</span>
                {foreign && <span className="ml-2 rounded bg-ink-800 px-1 font-semibold uppercase text-ink-300">{s.lang}</span>}
              </p>
              <p className="leading-relaxed">{s.text}</p>
              {foreign && s.translation && <p className="mt-1 leading-relaxed text-ink-300">→ {s.translation}</p>}
            </div>
          </div>
        );
      })}
    </div>
  );
}
