import { useEffect, useRef, useState } from "react";
import { api, errorText, events } from "../lib/api";
import { formatDuration } from "../lib/format";
import { useStickToBottom } from "../lib/useStickToBottom";
import { LANGUAGE_LABEL, type Bookmark, type LevelsEvent, type Meeting, type TrackLevel } from "../lib/types";
import { Button } from "../components/Button";
import { LevelMeter } from "../components/LevelMeter";
import { askConfirm } from "../components/ConfirmDialog";
import type { ActiveRecording } from "../App";

interface Props {
  recording: ActiveRecording;
  levels: LevelsEvent | null;
  onPausedChange: (paused: boolean) => void;
  onStopped: (m: Meeting) => void;
}

export function Live({ recording, levels, onPausedChange, onStopped }: Props) {
  const { meeting, paused, elapsedMs } = recording;
  const [bookmarks, setBookmarks] = useState<Bookmark[]>([]);
  const [errors, setErrors] = useState<string[]>([]);
  const [stopping, setStopping] = useState(false);
  const [flash, setFlash] = useState(false);
  const flashTimer = useRef<number>();
  const list = useStickToBottom<HTMLUListElement>(bookmarks.length);

  useEffect(() => {
    api.getMeeting(meeting.id).then((d) => setBookmarks(d.bookmarks)).catch(() => {});
    const un = events.onRecordingError((msg) => setErrors((e) => [...e.slice(-2), msg]));
    return () => {
      un.then((f) => f());
    };
  }, [meeting.id]);

  const fail = (e: unknown) => setErrors((x) => [...x.slice(-2), errorText(e)]);

  const togglePause = async () => {
    try {
      if (paused) await api.resumeMeeting();
      else await api.pauseMeeting();
      onPausedChange(!paused);
    } catch (e) {
      fail(e);
    }
  };

  const bookmark = async () => {
    try {
      const b = await api.addBookmark();
      setBookmarks((x) => [...x, b]);
      setFlash(true);
      window.clearTimeout(flashTimer.current);
      flashTimer.current = window.setTimeout(() => setFlash(false), 600);
    } catch (e) {
      fail(e);
    }
  };

  const stop = async () => {
    const ok = await askConfirm({
      title: "Dừng ghi âm?",
      message: "Cuộc họp sẽ được lưu lại. Không thể ghi tiếp vào cuộc họp này sau khi dừng.",
      confirmLabel: "Dừng & lưu",
      danger: true,
    });
    if (!ok) return;
    setStopping(true);
    try {
      onStopped(await api.stopMeeting());
    } catch (e) {
      fail(e);
      setStopping(false);
    }
  };

  // Phím tắt: Ctrl+B = đánh dấu.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key.toLowerCase() === "b") {
        e.preventDefault();
        bookmark();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const meters: TrackLevel[] = levels?.levels ?? [{ track: "mic", rms: 0, peak: 0 }];
  const sub = [meeting.project, meeting.client].filter(Boolean).join(" · ");

  return (
    <div className="flex h-full flex-col">
      {/* Thanh điều khiển */}
      <section className="flex items-center gap-8 border-b border-ink-800 px-8 py-5">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider">
            <span className={`h-2 w-2 rounded-full ${paused ? "bg-warn" : "rec-dot bg-rec"}`} />
            <span className={paused ? "text-amber-300" : "text-red-300"}>{paused ? "Tạm dừng" : "Đang ghi âm"}</span>
            <span className="text-ink-600">·</span>
            <span className="text-ink-400">{LANGUAGE_LABEL[meeting.language]}</span>
          </div>
          <h1 className="mt-1 truncate text-xl font-bold">{meeting.title}</h1>
          {sub && <p className="truncate text-sm text-ink-400">{sub}</p>}
        </div>

        <div className={`font-mono text-4xl font-semibold tabular-nums ${paused ? "text-ink-400" : "text-white"}`}>
          {formatDuration(elapsedMs)}
        </div>

        <div className="w-72 space-y-2">
          {meters.map((l) => (
            <LevelMeter key={l.track} track={l.track} rms={l.rms} peak={l.peak} active={!paused} />
          ))}
        </div>
      </section>

      <section className="flex items-center gap-2 border-b border-ink-800 px-8 py-3">
        <Button onClick={bookmark} title="Ctrl+B" className={flash ? "!bg-white !text-ink-950" : ""}>
          ★ Đánh dấu
        </Button>
        <Button disabled title="Có ở M4 (Ollama)">
          Tóm tắt nhanh tới giờ
        </Button>
        <div className="flex-1" />
        <Button onClick={togglePause} disabled={stopping}>
          {paused ? "▶ Tiếp tục" : "❚❚ Tạm dừng"}
        </Button>
        <Button variant="danger" onClick={stop} disabled={stopping}>
          ■ {stopping ? "Đang lưu…" : "Dừng & lưu"}
        </Button>
      </section>

      {errors.length > 0 && (
        <div className="space-y-1 border-b border-rec/30 bg-rec-soft px-8 py-2 text-sm text-red-200">
          {errors.map((e, i) => (
            <p key={i}>{e}</p>
          ))}
        </div>
      )}

      {/* Phụ đề + đánh dấu */}
      <section className="flex min-h-0 flex-1">
        <div className="grid min-w-0 flex-1 grid-cols-2 divide-x divide-ink-800">
          <Column title="Lời gốc" />
          <Column title="Bản dịch tiếng Việt" />
        </div>
        <aside className="flex w-72 shrink-0 flex-col border-l border-ink-800">
          <h2 className="px-5 pb-2 pt-4 text-xs font-semibold uppercase tracking-wider text-ink-400">
            Đánh dấu ({bookmarks.length})
          </h2>
          <ul ref={list.ref} onScroll={list.onScroll} className="min-h-0 flex-1 overflow-y-auto px-3 pb-4">
            {bookmarks.length === 0 && (
              <li className="px-2 py-2 text-sm text-ink-400">Bấm “Đánh dấu” (Ctrl+B) ở khoảnh khắc quan trọng.</li>
            )}
            {bookmarks.map((b) => (
              <BookmarkItem
                key={b.id}
                b={b}
                onChange={(label) => {
                  setBookmarks((x) => x.map((y) => (y.id === b.id ? { ...y, label } : y)));
                  api.updateBookmark(b.id, label).catch(fail);
                }}
              />
            ))}
          </ul>
        </aside>
      </section>
    </div>
  );
}

function Column({ title }: { title: string }) {
  return (
    <div className="flex min-h-0 flex-col">
      <h2 className="px-6 pb-2 pt-4 text-xs font-semibold uppercase tracking-wider text-ink-400">{title}</h2>
      <div className="flex flex-1 items-center justify-center px-6 text-center text-sm text-ink-400">
        <p>
          Phụ đề trực tiếp sẽ hiện ở đây từ mốc M3 (whisper.cpp).
          <br />
          Âm thanh đang được ghi an toàn vào máy.
        </p>
      </div>
    </div>
  );
}

function BookmarkItem({ b, onChange }: { b: Bookmark; onChange: (label: string) => void }) {
  const [label, setLabel] = useState(b.label);
  return (
    <li className="flex items-center gap-2 rounded-lg px-2 py-1.5 hover:bg-ink-850">
      <span className="font-mono text-xs text-ink-400">{formatDuration(b.atMs)}</span>
      <input
        value={label}
        onChange={(e) => setLabel(e.target.value)}
        onBlur={() => label !== b.label && onChange(label)}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
        placeholder="Ghi chú…"
        className="min-w-0 flex-1 bg-transparent text-sm placeholder:text-ink-600 focus:outline-none"
      />
    </li>
  );
}
