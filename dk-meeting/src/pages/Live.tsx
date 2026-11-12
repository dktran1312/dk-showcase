import { useEffect, useMemo, useRef, useState } from "react";
import { api, errorText, events, subscribe } from "../lib/api";
import { formatDuration } from "../lib/format";
import { useStickToBottom } from "../lib/useStickToBottom";
import {
  LANGUAGE_LABEL,
  TRACK_LABEL,
  type Bookmark,
  type LevelsEvent,
  type Meeting,
  type Segment,
  type SttStatus,
  type TrackLevel,
} from "../lib/types";
import { Button } from "../components/Button";
import { LevelMeter } from "../components/LevelMeter";
import { askConfirm } from "../components/ConfirmDialog";
import { SttBanner } from "../components/SttBanner";
import type { ActiveRecording } from "../App";

interface Props {
  recording: ActiveRecording;
  levels: LevelsEvent | null;
  onPausedChange: (paused: boolean) => void;
  onStopped: (m: Meeting) => void;
}

type AsideTab = "bookmarks" | "summary";

export function Live({ recording, levels, onPausedChange, onStopped }: Props) {
  const { meeting, paused, elapsedMs } = recording;
  const [bookmarks, setBookmarks] = useState<Bookmark[]>([]);
  const [segments, setSegments] = useState<Segment[]>([]);
  const [stt, setStt] = useState<SttStatus | null>(null);
  const [llmError, setLlmError] = useState<string>();
  const [translateOn, setTranslateOn] = useState(true);
  const [errors, setErrors] = useState<string[]>([]);
  const [stopping, setStopping] = useState(false);
  const [flash, setFlash] = useState(false);
  const [aside, setAside] = useState<AsideTab>("bookmarks");
  const [summary, setSummary] = useState<{ text?: string; at?: number; loading: boolean; error?: string }>({
    loading: false,
  });
  const flashTimer = useRef<number>();
  const translatedCount = useMemo(() => segments.filter((s) => s.translation).length, [segments]);
  const transcript = useStickToBottom<HTMLDivElement>(segments.length * 1000 + translatedCount);
  const list = useStickToBottom<HTMLUListElement>(bookmarks.length);
  const showTranslation = meeting.language !== "vi" && translateOn;

  useEffect(() => {
    api
      .getMeeting(meeting.id)
      .then((d) => {
        setBookmarks(d.bookmarks);
        setSegments(d.segments);
      })
      .catch(() => {});
    api.recordingStatus().then((s) => s?.stt && setStt(s.stt)).catch(() => {});
    api.getSettings().then((s) => setTranslateOn(s.translateEnabled)).catch(() => {});
    const offs = [
      subscribe(events.onRecordingError((msg) => setErrors((e) => [...e.slice(-2), msg]))),
      subscribe(
        events.onSegment((s) => {
          if (s.meetingId === meeting.id) setSegments((x) => (x.some((y) => y.id === s.id) ? x : [...x, s]));
        }),
      ),
      subscribe(
        events.onTranslation((t) => {
          if (t.meetingId !== meeting.id) return;
          setLlmError(undefined);
          setSegments((x) => x.map((s) => (s.id === t.segmentId ? { ...s, translation: t.translation } : s)));
        }),
      ),
      subscribe(events.onSttStatus((e) => e.meetingId === meeting.id && setStt(e.status))),
      subscribe(events.onLlmError((e) => e.meetingId === meeting.id && setLlmError(e.message))),
    ];
    return () => offs.forEach((f) => f());
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
      setAside("bookmarks");
      setFlash(true);
      window.clearTimeout(flashTimer.current);
      flashTimer.current = window.setTimeout(() => setFlash(false), 600);
    } catch (e) {
      fail(e);
    }
  };

  const quickSummary = async () => {
    setAside("summary");
    setSummary((s) => ({ ...s, loading: true, error: undefined }));
    try {
      const text = await api.quickSummary();
      setSummary({ text, at: elapsedMs, loading: false });
    } catch (e) {
      setSummary((s) => ({ ...s, loading: false, error: errorText(e) }));
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
            {stt?.state === "ready" && (
              <>
                <span className="text-ink-600">·</span>
                <span className="text-ink-400">Phụ đề {stt.gpu ? "GPU" : "CPU"}</span>
              </>
            )}
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
        <Button onClick={quickSummary} disabled={summary.loading}>
          {summary.loading ? "Đang tóm tắt…" : "✦ Tóm tắt nhanh tới giờ"}
        </Button>
        <div className="flex-1" />
        <Button onClick={togglePause} disabled={stopping}>
          {paused ? "▶ Tiếp tục" : "❚❚ Tạm dừng"}
        </Button>
        <Button variant="danger" onClick={stop} disabled={stopping}>
          ■ {stopping ? "Đang lưu…" : "Dừng & lưu"}
        </Button>
      </section>

      <SttBanner status={stt} />
      {errors.length > 0 && (
        <div className="space-y-1 border-b border-rec/30 bg-rec-soft px-8 py-2 text-sm text-red-200">
          {errors.map((e, i) => (
            <p key={i}>{e}</p>
          ))}
        </div>
      )}

      {/* Phụ đề + cột phải */}
      <section className="flex min-h-0 flex-1">
        <div className="relative flex min-w-0 flex-1 flex-col">
          <div
            className={`grid border-b border-ink-800 text-xs font-semibold uppercase tracking-wider text-ink-400 ${
              showTranslation ? "grid-cols-2 divide-x divide-ink-800" : "grid-cols-1"
            }`}
          >
            <h2 className="px-6 py-2.5">Lời gốc</h2>
            {showTranslation && (
              <h2 className="flex items-center justify-between px-6 py-2.5">
                Bản dịch tiếng Việt
                {llmError && (
                  <span className="normal-case tracking-normal text-amber-300" title={llmError}>
                    ⚠️ dịch đang tạm dừng
                  </span>
                )}
              </h2>
            )}
          </div>
          <div ref={transcript.ref} onScroll={transcript.onScroll} className="selectable min-h-0 flex-1 overflow-y-auto">
            {segments.length === 0 ? (
              <EmptyTranscript stt={stt} paused={paused} />
            ) : (
              segments.map((s) => <Row key={s.id} s={s} showTranslation={showTranslation} />)
            )}
          </div>
          {!transcript.stuck && (
            <button
              onClick={transcript.scrollToBottom}
              className="absolute bottom-4 left-1/2 -translate-x-1/2 rounded-full bg-white px-4 py-1.5 text-xs font-semibold text-ink-950 shadow-lg"
            >
              ↓ Xuống câu mới nhất
            </button>
          )}
        </div>

        <aside className="flex w-80 shrink-0 flex-col border-l border-ink-800">
          <div className="flex gap-1 border-b border-ink-800 px-3 pt-2">
            <AsideTabButton active={aside === "bookmarks"} onClick={() => setAside("bookmarks")}>
              Đánh dấu ({bookmarks.length})
            </AsideTabButton>
            <AsideTabButton active={aside === "summary"} onClick={() => setAside("summary")}>
              Tóm tắt nhanh
            </AsideTabButton>
          </div>
          {aside === "bookmarks" ? (
            <ul ref={list.ref} onScroll={list.onScroll} className="min-h-0 flex-1 overflow-y-auto px-3 py-3">
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
          ) : (
            <div className="selectable min-h-0 flex-1 overflow-y-auto px-5 py-4 text-sm">
              {summary.loading && <p className="text-ink-400">Đang tóm tắt bằng Ollama… (10–30 giây)</p>}
              {summary.error && <p className="text-rec">{summary.error}</p>}
              {!summary.loading && !summary.text && !summary.error && (
                <p className="text-ink-400">Bấm “Tóm tắt nhanh tới giờ” để xem các ý chính đã bàn.</p>
              )}
              {summary.text && (
                <>
                  <p className="mb-2 text-xs text-ink-400">Tới phút {formatDuration(summary.at ?? 0)}</p>
                  <div className="whitespace-pre-wrap leading-relaxed text-zinc-200">{summary.text}</div>
                </>
              )}
            </div>
          )}
        </aside>
      </section>
    </div>
  );
}

function Row({ s, showTranslation }: { s: Segment; showTranslation: boolean }) {
  const foreign = s.lang && s.lang !== "vi";
  return (
    <div
      className={`grid border-b border-ink-850 ${showTranslation ? "grid-cols-2 divide-x divide-ink-850" : "grid-cols-1"}`}
    >
      <div className="px-6 py-3">
        <div className="mb-1 flex items-center gap-2 text-[11px] text-ink-400">
          <span className="font-mono">{formatDuration(s.startMs)}</span>
          <span className={s.track === "mic" ? "text-sky-300" : "text-violet-300"}>{TRACK_LABEL[s.track]}</span>
          {foreign && <span className="rounded bg-ink-800 px-1 font-semibold uppercase">{s.lang}</span>}
        </div>
        <p className="leading-relaxed">{s.text}</p>
      </div>
      {showTranslation && (
        <div className="px-6 py-3">
          <div className="mb-1 h-[17px]" />
          {!foreign ? null : s.translation ? (
            <p className="leading-relaxed text-zinc-200">{s.translation}</p>
          ) : (
            <p className="text-sm text-ink-600">đang dịch…</p>
          )}
        </div>
      )}
    </div>
  );
}

function EmptyTranscript({ stt, paused }: { stt: SttStatus | null; paused: boolean }) {
  let text = "Đang nghe… phụ đề sẽ hiện sau khi người nói ngắt câu (2–5 giây).";
  if (paused) text = "Đang tạm dừng ghi âm.";
  else if (!stt || stt.state === "loading") text = "Đang chuẩn bị nhận giọng nói…";
  else if (stt.state !== "ready") text = "Phụ đề trực tiếp không khả dụng. Âm thanh vẫn được ghi an toàn vào máy.";
  return <div className="flex h-full items-center justify-center px-6 text-center text-sm text-ink-400">{text}</div>;
}

function AsideTabButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      className={`-mb-px border-b-2 px-3 py-2 text-xs font-semibold uppercase tracking-wider ${
        active ? "border-white text-white" : "border-transparent text-ink-400 hover:text-white"
      }`}
    >
      {children}
    </button>
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
