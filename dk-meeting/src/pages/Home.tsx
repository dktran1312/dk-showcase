import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "../lib/api";
import { formatDayGroup, formatDuration, formatTime } from "../lib/format";
import { STATUS_LABEL, type Meeting } from "../lib/types";
import { Button } from "../components/Button";
import type { ActiveRecording } from "../App";

interface Props {
  recording: ActiveRecording | null;
  onNew: () => void;
  onOpen: (id: string) => void;
}

export function Home({ recording, onNew, onOpen }: Props) {
  const [query, setQuery] = useState("");
  const [meetings, setMeetings] = useState<Meeting[] | null>(null);
  const [error, setError] = useState<string>();

  useEffect(() => {
    const t = setTimeout(() => {
      api
        .listMeetings(query)
        .then((m) => {
          setMeetings(m);
          setError(undefined);
        })
        .catch((e) => setError(errorText(e)));
    }, query ? 250 : 0);
    return () => clearTimeout(t);
  }, [query, recording?.meeting.id]);

  const groups = useMemo(() => {
    const out: { day: string; items: Meeting[] }[] = [];
    for (const m of meetings ?? []) {
      const day = formatDayGroup(m.startedAt);
      const last = out[out.length - 1];
      if (last && last.day === day) last.items.push(m);
      else out.push({ day, items: [m] });
    }
    return out;
  }, [meetings]);

  return (
    <div className="flex h-full flex-col">
      <section className="border-b border-ink-800 px-10 pb-8 pt-10">
        <div className="mx-auto flex max-w-5xl items-end justify-between gap-8">
          <div>
            <h1 className="text-3xl font-extrabold tracking-tight">Cuộc họp</h1>
            <p className="mt-1 text-sm text-ink-400">Ghi âm, phụ đề và biên bản — tất cả chạy trên máy này.</p>
          </div>
          <Button variant="record" onClick={onNew} className="h-14 rounded-2xl px-8 text-base">
            <span className={`h-3 w-3 rounded-full bg-white ${recording ? "rec-dot" : ""}`} />
            {recording ? "Quay lại cuộc họp đang ghi" : "Bắt đầu họp"}
          </Button>
        </div>
        <div className="mx-auto mt-8 max-w-5xl">
          <div className="relative">
            <svg
              className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-ink-400"
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <circle cx="11" cy="11" r="7" />
              <path d="m20 20-3.5-3.5" />
            </svg>
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Tìm theo tên cuộc họp, dự án, khách hàng hoặc nội dung đã nói…"
              className="h-11 w-full rounded-xl border border-ink-700 bg-ink-900 pl-11 pr-4 text-sm placeholder:text-ink-400 focus:border-ink-400 focus:outline-none"
            />
          </div>
        </div>
      </section>

      <section className="min-h-0 flex-1 overflow-y-auto px-10 py-6">
        <div className="mx-auto max-w-5xl">
          {error && <p className="text-sm text-rec">{error}</p>}
          {meetings && meetings.length === 0 && <Empty searching={!!query} />}
          {groups.map((g) => (
            <div key={g.day} className="mb-6">
              <h2 className="mb-2 text-xs font-semibold uppercase tracking-wider text-ink-400">{g.day}</h2>
              <ul className="overflow-hidden rounded-xl border border-ink-800 bg-ink-900">
                {g.items.map((m) => (
                  <li key={m.id} className="border-b border-ink-800 last:border-b-0">
                    <MeetingRow m={m} live={recording?.meeting.id === m.id} onOpen={() => onOpen(m.id)} />
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

function MeetingRow({ m, live, onOpen }: { m: Meeting; live: boolean; onOpen: () => void }) {
  const sub = [m.project, m.client].filter(Boolean).join(" · ");
  return (
    <button onClick={onOpen} className="flex w-full items-center gap-6 px-5 py-4 text-left hover:bg-ink-850">
      <span className="w-14 shrink-0 font-mono text-sm text-ink-400">{formatTime(m.startedAt)}</span>
      <span className="min-w-0 flex-1">
        <span className="block truncate font-semibold">{m.title}</span>
        <span className="block truncate text-sm text-ink-400">{sub || "Chưa gắn dự án / khách hàng"}</span>
      </span>
      {m.bookmarkCount > 0 && (
        <span className="shrink-0 text-xs text-ink-400" title="Số đánh dấu">
          ★ {m.bookmarkCount}
        </span>
      )}
      <StatusBadge status={live ? "recording" : m.status} />
      <span className="w-16 shrink-0 text-right font-mono text-sm text-ink-300">
        {live ? "—" : formatDuration(m.durationMs)}
      </span>
    </button>
  );
}

function StatusBadge({ status }: { status: Meeting["status"] }) {
  if (status === "done") return null;
  const cls =
    status === "interrupted"
      ? "border-warn/40 text-amber-300"
      : "border-rec/40 text-red-300";
  return (
    <span className={`shrink-0 rounded-full border px-2.5 py-0.5 text-xs font-medium ${cls}`}>
      {STATUS_LABEL[status]}
    </span>
  );
}

function Empty({ searching }: { searching: boolean }) {
  return (
    <div className="flex flex-col items-center py-24 text-center">
      <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl border border-ink-700 text-2xl">
        {searching ? "🔍" : "🎙️"}
      </div>
      <p className="font-semibold">{searching ? "Không tìm thấy cuộc họp nào" : "Chưa có cuộc họp nào"}</p>
      <p className="mt-1 text-sm text-ink-400">
        {searching ? "Thử từ khoá khác, có thể gõ không dấu." : "Bấm “Bắt đầu họp” để ghi âm cuộc họp đầu tiên."}
      </p>
    </div>
  );
}
