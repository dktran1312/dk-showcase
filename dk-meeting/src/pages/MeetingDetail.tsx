import { useEffect, useRef, useState } from "react";
import { api, errorText, events, subscribe } from "../lib/api";
import { formatDateTime, formatDuration } from "../lib/format";
import {
  LANGUAGE_LABEL,
  STATUS_LABEL,
  TRACK_LABEL,
  type Minutes,
  type MinutesProgress,
  type MeetingDetail,
  type Track,
} from "../lib/types";
import { Button } from "../components/Button";
import { askConfirm } from "../components/ConfirmDialog";
import { AudioPlayer, type PlayerHandle } from "../components/AudioPlayer";
import { MinutesView } from "../components/MinutesView";
import { ActionItemsTable } from "../components/ActionItemsTable";
import { TranscriptList } from "../components/TranscriptList";
import { ProgressBar } from "../components/ProgressBar";
import { TimeLink } from "../components/TimeLink";

type Tab = "minutes" | "actions" | "transcript" | "audio";

export function MeetingDetailPage({ id, onBack }: { id: string; onBack: () => void }) {
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [error, setError] = useState<string>();
  const [tab, setTab] = useState<Tab | null>(null);
  const [gen, setGen] = useState<MinutesProgress | null>(null);
  const [generating, setGenerating] = useState(false);
  const [transcribing, setTranscribing] = useState<{ doneMs: number; totalMs: number } | null>(null);
  const player = useRef<PlayerHandle>(null);

  const load = () =>
    api
      .getMeeting(id)
      .then((d) => {
        setDetail(d);
        setTab((t) => t ?? (d.minutes ? "minutes" : d.segments.length ? "transcript" : "audio"));
      })
      .catch((e) => setError(errorText(e)));

  useEffect(() => {
    load();
    const offs = [
      subscribe(
        events.onProcessed((e) => {
          if (e.meetingId !== id) return;
          setTranscribing(null);
          load();
        }),
      ),
      subscribe(events.onMinutesProgress((p) => p.meetingId === id && setGen(p))),
      subscribe(events.onTranscribeProgress((p) => p.meetingId === id && setTranscribing(p))),
      // câu mới / bản dịch khi đang xử lý nốt hoặc đang tạo transcript từ file
      subscribe(
        events.onSegment((s) => {
          if (s.meetingId !== id) return;
          setDetail((d) => (d ? { ...d, segments: [...d.segments, s].sort((a, b) => a.startMs - b.startMs) } : d));
        }),
      ),
      subscribe(
        events.onTranslation((t) => {
          if (t.meetingId !== id) return;
          setDetail((d) =>
            d
              ? { ...d, segments: d.segments.map((s) => (s.id === t.segmentId ? { ...s, translation: t.translation } : s)) }
              : d,
          );
        }),
      ),
    ];
    return () => offs.forEach((f) => f());
  }, [id]);

  if (error && !detail) return <p className="p-10 text-rec">{error}</p>;
  if (!detail || !tab) return null;
  const m = detail.meeting;
  const play = (ms: number, track?: Track) => player.current?.seek(ms, track);

  const save = (patch: Parameters<typeof api.updateMeeting>[1]) =>
    api
      .updateMeeting(id, patch)
      .then((meeting) => setDetail((d) => (d ? { ...d, meeting } : d)))
      .catch((e) => setError(errorText(e)));

  const remove = async () => {
    const ok = await askConfirm({
      title: "Xoá cuộc họp?",
      message: `“${m.title}” và toàn bộ file ghi âm sẽ bị xoá khỏi máy. Không thể hoàn tác.`,
      confirmLabel: "Xoá",
      danger: true,
    });
    if (!ok) return;
    try {
      await api.deleteMeeting(id);
      onBack();
    } catch (e) {
      setError(errorText(e));
    }
  };

  const generate = async () => {
    if (detail.minutes || detail.actionItems.length) {
      const ok = await askConfirm({
        title: "Tạo lại biên bản?",
        message:
          "Biên bản và danh sách việc cần làm hiện tại (kể cả phần đã sửa tay) sẽ được thay bằng bản mới tạo từ transcript.",
        confirmLabel: "Tạo lại",
      });
      if (!ok) return;
    }
    setError(undefined);
    setGenerating(true);
    setGen(null);
    setTab("minutes");
    try {
      const r = await api.generateMinutes(id);
      setDetail((d) => (d ? { ...d, minutes: r.minutes, actionItems: r.actionItems } : d));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setGenerating(false);
      setGen(null);
    }
  };

  const transcribe = async () => {
    if (detail.segments.length) {
      const ok = await askConfirm({
        title: "Nhận dạng lại từ file ghi âm?",
        message: "Transcript hiện tại sẽ bị thay bằng bản nhận dạng mới (dùng model đang chọn trong Cài đặt).",
        confirmLabel: "Nhận dạng lại",
      });
      if (!ok) return;
    }
    setError(undefined);
    setTranscribing({ doneMs: 0, totalMs: m.durationMs });
    setDetail((d) => (d ? { ...d, segments: [] } : d));
    setTab("transcript");
    api.transcribeMeeting(id).catch((e) => {
      setError(errorText(e));
      setTranscribing(null);
      load();
    });
  };

  const busy = detail.processing || !!transcribing;
  const tabs: { key: Tab; label: string }[] = [
    { key: "minutes", label: "Biên bản" },
    { key: "actions", label: `Việc cần làm${detail.actionItems.length ? ` (${detail.actionItems.length})` : ""}` },
    { key: "transcript", label: `Transcript${detail.segments.length ? ` (${detail.segments.length})` : ""}` },
    { key: "audio", label: "Âm thanh" },
  ];

  return (
    <div className="flex h-full flex-col">
      <section className="border-b border-ink-800 px-10 pb-0 pt-8">
        <div className="mx-auto max-w-5xl">
          <button onClick={onBack} className="mb-4 text-sm text-ink-400 hover:text-white">
            ← Cuộc họp
          </button>
          <div className="flex items-start gap-6">
            <div className="min-w-0 flex-1">
              <EditableText value={m.title} onSave={(title) => save({ title })} className="text-2xl font-extrabold tracking-tight" />
              <div className="mt-1 flex gap-2 text-sm">
                <EditableText value={m.project} placeholder="+ Dự án" onSave={(project) => save({ project })} className="text-ink-300" />
                <span className="text-ink-600">·</span>
                <EditableText value={m.client} placeholder="+ Khách hàng" onSave={(client) => save({ client })} className="text-ink-300" />
              </div>
              <p className="mt-3 text-sm text-ink-400">
                {formatDateTime(m.startedAt)} · {formatDuration(m.durationMs)} · {LANGUAGE_LABEL[m.language]}
                {m.status !== "done" && <span className="ml-2 text-amber-300">· {STATUS_LABEL[m.status]}</span>}
              </p>
            </div>
            <div className="flex shrink-0 gap-2">
              <Button
                variant="primary"
                onClick={generate}
                disabled={generating || busy || detail.segments.length === 0}
                title={detail.segments.length === 0 ? "Cần có transcript trước" : undefined}
              >
                {generating ? "Đang viết biên bản…" : detail.minutes ? "Tạo lại biên bản" : "✦ Tạo biên bản"}
              </Button>
              <Button disabled title="Có ở M5">
                Xuất file
              </Button>
              <Button variant="ghost" onClick={() => api.revealInFolder(detail.folder).catch((e) => setError(errorText(e)))}>
                Mở thư mục
              </Button>
              <Button variant="danger" onClick={remove} disabled={busy}>
                Xoá
              </Button>
            </div>
          </div>
          {m.status === "interrupted" && (
            <p className="mt-4 rounded-lg border border-warn/30 bg-warn/10 px-4 py-2 text-sm text-amber-200">
              App đã bị tắt đột ngột trong lúc ghi. Phần âm thanh đã ghi được vẫn được giữ lại.
            </p>
          )}
          {detail.processing && !transcribing && (
            <p className="mt-4 rounded-lg border border-ink-700 bg-ink-900 px-4 py-2 text-sm text-ink-300">
              <span className="rec-dot mr-2 inline-block h-2 w-2 rounded-full bg-white" />
              Đang nhận dạng và dịch nốt những câu cuối…
            </p>
          )}
          {error && <p className="mt-4 rounded-lg bg-rec-soft px-4 py-2 text-sm text-red-200">{error}</p>}
          <nav className="mt-6 flex gap-1">
            {tabs.map((t) => (
              <button
                key={t.key}
                onClick={() => setTab(t.key)}
                className={`-mb-px border-b-2 px-4 py-2.5 text-sm font-medium ${
                  tab === t.key ? "border-white text-white" : "border-transparent text-ink-400 hover:text-white"
                }`}
              >
                {t.label}
              </button>
            ))}
          </nav>
        </div>
      </section>

      <section className="min-h-0 flex-1 overflow-y-auto px-10 py-8">
        <div className="mx-auto max-w-5xl">
          {tab === "minutes" &&
            (generating ? (
              <div className="rounded-xl border border-ink-800 bg-ink-900 px-6 py-10">
                <p className="mb-4 text-sm">{gen?.message ?? "Đang gửi transcript cho Ollama…"}</p>
                <ProgressBar value={gen?.step ?? 0} total={gen && gen.total > 1 ? gen.total : 0} />
                <p className="mt-4 text-xs text-ink-400">
                  Cuộc họp dài được chia thành từng đoạn ~10 phút để tóm tắt rồi gộp lại. Có thể mất vài phút.
                </p>
              </div>
            ) : detail.minutes ? (
              <MinutesView
                record={detail.minutes}
                actionItems={detail.actionItems}
                onPlay={play}
                onSave={async (data: Minutes) => {
                  const r = await api.saveMinutes(id, data);
                  setDetail((d) => (d ? { ...d, minutes: r } : d));
                }}
              />
            ) : (
              <Empty
                title="Chưa có biên bản"
                text={
                  detail.segments.length
                    ? "Bấm “Tạo biên bản” để AI (Ollama, chạy trên máy) viết biên bản theo mẫu ngành sự kiện từ transcript."
                    : "Cần có transcript trước. Vào tab Transcript để tạo từ file ghi âm."
                }
              />
            ))}

          {tab === "actions" && (
            <ActionItemsTable
              items={detail.actionItems}
              onPlay={play}
              onSave={async (items) => {
                const saved = await api.saveActionItems(id, items);
                setDetail((d) => (d ? { ...d, actionItems: saved } : d));
              }}
            />
          )}

          {tab === "transcript" && (
            <div>
              <div className="mb-3 flex items-center justify-between gap-4">
                <p className="text-sm text-ink-400">
                  {transcribing
                    ? `Đang nhận dạng từ file ghi âm… ${formatDuration(transcribing.doneMs)} / ${formatDuration(transcribing.totalMs)}`
                    : "Bấm vào câu để nghe lại đúng chỗ."}
                </p>
                {detail.chunks.length > 0 && (
                  <Button variant="ghost" onClick={transcribe} disabled={busy}>
                    {detail.segments.length ? "Nhận dạng lại từ file ghi âm" : "Tạo transcript từ file ghi âm"}
                  </Button>
                )}
              </div>
              {transcribing && (
                <div className="mb-4">
                  <ProgressBar value={transcribing.doneMs} total={transcribing.totalMs} />
                </div>
              )}
              {detail.segments.length ? (
                <TranscriptList segments={detail.segments} onPlay={play} />
              ) : (
                !transcribing && (
                  <Empty
                    title="Chưa có transcript"
                    text="Cuộc họp này được ghi khi chưa bật phụ đề (hoặc chưa có model). Bấm “Tạo transcript từ file ghi âm” để nhận dạng lại."
                  />
                )
              )}
            </div>
          )}

          {tab === "audio" && <AudioTab detail={detail} onPlay={play} />}
        </div>
      </section>

      <AudioPlayer ref={player} chunks={detail.chunks} />
    </div>
  );
}

function Empty({ title, text }: { title: string; text: string }) {
  return (
    <div className="rounded-xl border border-dashed border-ink-700 px-6 py-14 text-center">
      <p className="font-semibold">{title}</p>
      <p className="mx-auto mt-1 max-w-md text-sm text-ink-400">{text}</p>
    </div>
  );
}

function AudioTab({ detail, onPlay }: { detail: MeetingDetail; onPlay: (ms: number, track?: Track) => void }) {
  if (detail.chunks.length === 0) {
    return <p className="text-sm text-ink-400">Cuộc họp này không có file âm thanh nào.</p>;
  }
  return (
    <div className="grid grid-cols-[1fr_18rem] gap-8">
      <ul className="space-y-1">
        {detail.chunks.map((c) => (
          <li key={c.path}>
            <button
              onClick={() => onPlay(c.startMs, c.track)}
              className="flex w-full items-center gap-4 rounded-lg px-3 py-2 text-left text-sm text-ink-300 hover:bg-ink-900"
            >
              <span className="font-mono">{formatDuration(c.startMs)}</span>
              <span className="text-xs text-ink-400">{TRACK_LABEL[c.track]}</span>
              <span className="flex-1 truncate">{c.fileName}</span>
              <span className="font-mono text-ink-400">{formatDuration(c.durationMs)}</span>
            </button>
          </li>
        ))}
      </ul>
      <aside>
        <h2 className="mb-2 text-xs font-semibold uppercase tracking-wider text-ink-400">Đánh dấu ({detail.bookmarks.length})</h2>
        {detail.bookmarks.length === 0 && <p className="text-sm text-ink-400">Không có đánh dấu.</p>}
        <ul className="space-y-1">
          {detail.bookmarks.map((b) => (
            <li key={b.id} className="flex items-center gap-2 text-sm">
              <TimeLink ms={b.atMs} onPlay={(ms) => onPlay(ms)} />
              <span className="flex-1 truncate">{b.label || "★"}</span>
            </li>
          ))}
        </ul>
      </aside>
    </div>
  );
}

function EditableText({
  value,
  onSave,
  className = "",
  placeholder,
}: {
  value: string;
  onSave: (v: string) => void;
  className?: string;
  placeholder?: string;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);

  if (editing) {
    return (
      <input
        autoFocus
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => {
          setEditing(false);
          if (draft !== value) onSave(draft);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") (e.target as HTMLInputElement).blur();
          if (e.key === "Escape") {
            setDraft(value);
            setEditing(false);
          }
        }}
        className={`rounded bg-ink-850 px-1 focus:outline-none ${className}`}
      />
    );
  }
  return (
    <button onClick={() => setEditing(true)} title="Bấm để sửa" className={`rounded px-1 -mx-1 text-left hover:bg-ink-850 ${className}`}>
      {value || <span className="text-ink-600">{placeholder}</span>}
    </button>
  );
}
