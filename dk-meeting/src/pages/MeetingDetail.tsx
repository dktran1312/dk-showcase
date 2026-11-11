import { useEffect, useMemo, useRef, useState } from "react";
import { api, errorText } from "../lib/api";
import { formatDateTime, formatDuration } from "../lib/format";
import { LANGUAGE_LABEL, STATUS_LABEL, TRACK_LABEL, type AudioChunk, type MeetingDetail, type Track } from "../lib/types";
import { Button } from "../components/Button";
import { askConfirm } from "../components/ConfirmDialog";

type Tab = "minutes" | "actions" | "transcript" | "audio";

const TABS: { key: Tab; label: string; milestone?: string }[] = [
  { key: "minutes", label: "Biên bản", milestone: "M4" },
  { key: "actions", label: "Việc cần làm", milestone: "M4" },
  { key: "transcript", label: "Transcript", milestone: "M3" },
  { key: "audio", label: "Âm thanh" },
];

export function MeetingDetailPage({ id, onBack }: { id: string; onBack: () => void }) {
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [error, setError] = useState<string>();
  const [tab, setTab] = useState<Tab>("audio");

  useEffect(() => {
    api.getMeeting(id).then(setDetail).catch((e) => setError(errorText(e)));
  }, [id]);

  if (error) return <p className="p-10 text-rec">{error}</p>;
  if (!detail) return null;
  const m = detail.meeting;

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

  return (
    <div className="flex h-full flex-col">
      <section className="border-b border-ink-800 px-10 pb-0 pt-8">
        <div className="mx-auto max-w-5xl">
          <button onClick={onBack} className="mb-4 text-sm text-ink-400 hover:text-white">
            ← Cuộc họp
          </button>
          <div className="flex items-start gap-6">
            <div className="min-w-0 flex-1">
              <EditableText
                value={m.title}
                onSave={(title) => save({ title })}
                className="text-2xl font-extrabold tracking-tight"
              />
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
            <div className="flex gap-2">
              <Button disabled title="Có ở M4">Tạo lại biên bản</Button>
              <Button disabled title="Có ở M5">Xuất file</Button>
              <Button variant="ghost" onClick={() => api.revealInFolder(detail.folder).catch((e) => setError(errorText(e)))}>
                Mở thư mục
              </Button>
              <Button variant="danger" onClick={remove}>
                Xoá
              </Button>
            </div>
          </div>
          {m.status === "interrupted" && (
            <p className="mt-4 rounded-lg border border-warn/30 bg-warn/10 px-4 py-2 text-sm text-amber-200">
              App đã bị tắt đột ngột trong lúc ghi. Phần âm thanh đã ghi được vẫn được giữ lại bên dưới.
            </p>
          )}
          <nav className="mt-6 flex gap-1">
            {TABS.map((t) => (
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
          {tab === "audio" ? (
            <AudioTab detail={detail} />
          ) : (
            <Placeholder milestone={TABS.find((t) => t.key === tab)?.milestone ?? ""} />
          )}
        </div>
      </section>
    </div>
  );
}

function Placeholder({ milestone }: { milestone: string }) {
  return (
    <div className="rounded-xl border border-dashed border-ink-700 px-6 py-16 text-center text-sm text-ink-400">
      Phần này sẽ có ở mốc {milestone}.
    </div>
  );
}

function AudioTab({ detail }: { detail: MeetingDetail }) {
  const tracks = useMemo(() => {
    const map = new Map<Track, AudioChunk[]>();
    for (const c of detail.chunks) map.set(c.track, [...(map.get(c.track) ?? []), c]);
    return map;
  }, [detail.chunks]);
  const trackKeys = [...tracks.keys()];
  const [track, setTrack] = useState<Track>(trackKeys[0] ?? "mic");
  const chunks = tracks.get(track) ?? [];
  const [current, setCurrent] = useState(0);
  const audio = useRef<HTMLAudioElement>(null);
  const pendingSeek = useRef<number | null>(null);
  const [playError, setPlayError] = useState<string>();

  const playAt = (ms: number) => {
    const i = chunks.findIndex((c) => ms >= c.startMs && ms < c.startMs + Math.max(c.durationMs, 1));
    const idx = i >= 0 ? i : chunks.length - 1;
    if (idx < 0) return;
    const offset = Math.max(0, (ms - chunks[idx].startMs) / 1000);
    if (idx === current && audio.current) {
      audio.current.currentTime = offset;
      audio.current.play();
    } else {
      pendingSeek.current = offset;
      setCurrent(idx);
    }
  };

  if (detail.chunks.length === 0) {
    return <p className="text-sm text-ink-400">Cuộc họp này không có file âm thanh nào.</p>;
  }
  const chunk = chunks[current];

  return (
    <div className="grid grid-cols-[1fr_18rem] gap-8">
      <div>
        {trackKeys.length > 1 && (
          <div className="mb-4 flex gap-2">
            {trackKeys.map((t) => (
              <Button key={t} variant={t === track ? "primary" : "secondary"} onClick={() => { setTrack(t); setCurrent(0); }}>
                {TRACK_LABEL[t]}
              </Button>
            ))}
          </div>
        )}
        <div className="rounded-xl border border-ink-800 bg-ink-900 p-5">
          <p className="mb-3 text-sm text-ink-300">
            {TRACK_LABEL[track]} · đoạn {current + 1}/{chunks.length} ·{" "}
            <span className="font-mono">
              {formatDuration(chunk.startMs)}–{formatDuration(chunk.startMs + chunk.durationMs)}
            </span>
          </p>
          <audio
            ref={audio}
            key={chunk.path}
            src={api.fileSrc(chunk.path)}
            controls
            className="w-full"
            onError={(e) => {
              const err = e.currentTarget.error;
              setPlayError(`Không phát được file (${err?.code ?? "?"}${err?.message ? `: ${err.message}` : ""})`);
            }}
            onLoadedMetadata={(e) => {
              setPlayError(undefined);
              if (pendingSeek.current !== null) {
                e.currentTarget.currentTime = pendingSeek.current;
                pendingSeek.current = null;
                e.currentTarget.play();
              }
            }}
            onEnded={() => {
              if (current < chunks.length - 1) {
                pendingSeek.current = 0;
                setCurrent(current + 1);
              }
            }}
          />
          {playError && <p className="mt-2 text-xs text-rec">{playError}</p>}
        </div>
        <ul className="mt-4 space-y-1">
          {chunks.map((c, i) => (
            <li key={c.path}>
              <button
                onClick={() => playAt(c.startMs)}
                className={`flex w-full items-center gap-4 rounded-lg px-3 py-2 text-left text-sm ${
                  i === current ? "bg-ink-850 text-white" : "text-ink-300 hover:bg-ink-900"
                }`}
              >
                <span className="font-mono">{formatDuration(c.startMs)}</span>
                <span className="flex-1 truncate">{c.fileName}</span>
                <span className="font-mono text-ink-400">{formatDuration(c.durationMs)}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>

      <aside>
        <h2 className="mb-2 text-xs font-semibold uppercase tracking-wider text-ink-400">
          Đánh dấu ({detail.bookmarks.length})
        </h2>
        {detail.bookmarks.length === 0 && <p className="text-sm text-ink-400">Không có đánh dấu.</p>}
        <ul className="space-y-1">
          {detail.bookmarks.map((b) => (
            <li key={b.id}>
              <button
                onClick={() => playAt(b.atMs)}
                className="flex w-full gap-3 rounded-lg px-2 py-1.5 text-left text-sm hover:bg-ink-900"
                title="Bấm để nghe lại"
              >
                <span className="font-mono text-ink-400">{formatDuration(b.atMs)}</span>
                <span className="flex-1 truncate">{b.label || "★"}</span>
              </button>
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
