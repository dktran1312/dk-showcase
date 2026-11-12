import { forwardRef, useImperativeHandle, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import { formatDuration } from "../lib/format";
import { TRACK_LABEL, type AudioChunk, type Track } from "../lib/types";

export interface PlayerHandle {
  /** Phát từ thời điểm `ms` (tính trên toàn cuộc họp), tuỳ chọn track. */
  seek: (ms: number, track?: Track) => void;
}

/** Thanh phát âm thanh dùng chung cho trang chi tiết; các file WAV 5 phút được nối liền mạch. */
export const AudioPlayer = forwardRef<PlayerHandle, { chunks: AudioChunk[] }>(function AudioPlayer({ chunks }, ref) {
  const tracks = useMemo(() => {
    const map = new Map<Track, AudioChunk[]>();
    for (const c of chunks) map.set(c.track, [...(map.get(c.track) ?? []), c]);
    return map;
  }, [chunks]);
  const trackKeys = [...tracks.keys()];
  const [track, setTrack] = useState<Track>(trackKeys[0] ?? "mic");
  const [current, setCurrent] = useState(0);
  const [error, setError] = useState<string>();
  const audio = useRef<HTMLAudioElement>(null);
  const pendingSeek = useRef<number | null>(null);
  const list = tracks.get(track) ?? [];

  useImperativeHandle(ref, () => ({
    seek(ms: number, t?: Track) {
      const useTrack = t && tracks.has(t) ? t : track;
      const cs = tracks.get(useTrack) ?? [];
      const i = cs.findIndex((c) => ms >= c.startMs && ms < c.startMs + Math.max(c.durationMs, 1));
      const idx = i >= 0 ? i : ms < (cs[0]?.startMs ?? 0) ? 0 : cs.length - 1;
      if (idx < 0) return;
      const offset = Math.max(0, (ms - cs[idx].startMs) / 1000);
      if (useTrack === track && idx === current && audio.current) {
        audio.current.currentTime = offset;
        audio.current.play().catch(() => {});
      } else {
        pendingSeek.current = offset;
        setTrack(useTrack);
        setCurrent(idx);
      }
    },
  }));

  if (chunks.length === 0) return null;
  const chunk = list[Math.min(current, list.length - 1)];
  if (!chunk) return null;

  return (
    <div className="flex items-center gap-4 border-t border-ink-800 bg-ink-900 px-10 py-3">
      {trackKeys.length > 1 && (
        <select
          value={track}
          onChange={(e) => {
            setTrack(e.target.value as Track);
            setCurrent(0);
          }}
          className="h-9 rounded-lg border border-ink-700 bg-ink-900 px-2 text-sm"
        >
          {trackKeys.map((t) => (
            <option key={t} value={t}>
              {TRACK_LABEL[t]}
            </option>
          ))}
        </select>
      )}
      <span className="w-28 shrink-0 font-mono text-xs text-ink-400" title={chunk.fileName}>
        {formatDuration(chunk.startMs)}–{formatDuration(chunk.startMs + chunk.durationMs)}
      </span>
      <audio
        ref={audio}
        key={chunk.path}
        src={api.fileSrc(chunk.path)}
        controls
        className="h-9 flex-1"
        onError={(e) => {
          const err = e.currentTarget.error;
          setError(`Không phát được file (${err?.code ?? "?"})`);
        }}
        onLoadedMetadata={(e) => {
          setError(undefined);
          if (pendingSeek.current !== null) {
            e.currentTarget.currentTime = pendingSeek.current;
            pendingSeek.current = null;
            e.currentTarget.play().catch(() => {});
          }
        }}
        onEnded={() => {
          if (current < list.length - 1) {
            pendingSeek.current = 0;
            setCurrent(current + 1);
          }
        }}
      />
      {error && <span className="text-xs text-rec">{error}</span>}
    </div>
  );
});
