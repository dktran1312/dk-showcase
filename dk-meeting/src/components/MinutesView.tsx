import { useEffect, useState } from "react";
import { parseTime } from "../lib/format";
import type { ActionItem, Minutes, MinutesRecord, TechItem } from "../lib/types";
import { ACTION_STATUS_LABEL } from "../lib/types";
import { Button } from "./Button";
import { TimeLink } from "./TimeLink";

const NA = "Chưa đề cập";

const TECH: { key: keyof Minutes["technical"]; label: string }[] = [
  { key: "led_visual", label: "LED / Visual" },
  { key: "sound", label: "Âm thanh" },
  { key: "lighting", label: "Ánh sáng" },
  { key: "sfx", label: "SFX" },
  { key: "stage", label: "Sân khấu / Kết cấu" },
  { key: "artist_mc", label: "Nghệ sĩ / MC" },
  { key: "other", label: "Khác" },
];

const BRIEF: { key: keyof Minutes["brief"]; label: string }[] = [
  { key: "objective", label: "Mục tiêu sự kiện" },
  { key: "concept", label: "Concept" },
  { key: "key_message", label: "Key message" },
  { key: "audience", label: "Đối tượng khán giả" },
  { key: "show_date", label: "Ngày show" },
  { key: "venue", label: "Địa điểm" },
  { key: "scale", label: "Quy mô" },
];

const empty = (s?: string) => !s || !s.trim() || s.trim() === NA;

interface Props {
  record: MinutesRecord;
  actionItems: ActionItem[];
  onPlay: (ms: number) => void;
  onSave: (m: Minutes) => Promise<void>;
}

/** Hiển thị biên bản theo mẫu ngành sự kiện; bấm “Sửa” để chỉnh trực tiếp. */
export function MinutesView({ record, actionItems, onPlay, onSave }: Props) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<Minutes>(record.data);
  const [saving, setSaving] = useState(false);
  useEffect(() => setDraft(record.data), [record]);
  const m = editing ? draft : record.data;

  const save = async () => {
    setSaving(true);
    try {
      await onSave(draft);
      setEditing(false);
    } finally {
      setSaving(false);
    }
  };

  const set = (patch: Partial<Minutes>) => setDraft((d) => ({ ...d, ...patch }));

  return (
    <div className="selectable space-y-6 pb-10">
      <div className="flex items-center justify-between">
        <p className="text-xs text-ink-400">
          {record.edited ? "Đã chỉnh sửa tay" : `Tạo bởi ${record.model}`} · Chỉ dựa trên transcript, hãy kiểm tra lại
          trước khi gửi khách.
        </p>
        {editing ? (
          <div className="flex gap-2">
            <Button
              variant="ghost"
              onClick={() => {
                setDraft(record.data);
                setEditing(false);
              }}
            >
              Huỷ
            </Button>
            <Button variant="primary" onClick={save} disabled={saving}>
              {saving ? "Đang lưu…" : "Lưu"}
            </Button>
          </div>
        ) : (
          <Button onClick={() => setEditing(true)}>✎ Sửa biên bản</Button>
        )}
      </div>

      <Section n={1} title="Thông tin chung">
        <dl className="grid grid-cols-[10rem_1fr] gap-x-4 gap-y-1.5 text-sm">
          <Dt>Cuộc họp</Dt>
          <dd>{m.general.title}</dd>
          <Dt>Thời gian</Dt>
          <dd>
            {m.general.date} · {m.general.duration}
          </dd>
          <Dt>Người tham gia</Dt>
          <dd>
            {editing ? (
              <TextInput
                value={draft.general.participants.join(", ")}
                onChange={(v) =>
                  set({ general: { ...draft.general, participants: v.split(",").map((s) => s.trim()).filter(Boolean) } })
                }
              />
            ) : (
              <Val v={m.general.participants.join(", ")} />
            )}
          </dd>
          <Dt>Khách hàng / nhãn hàng</Dt>
          <dd>
            {editing ? (
              <TextInput value={draft.general.client} onChange={(v) => set({ general: { ...draft.general, client: v } })} />
            ) : (
              <Val v={m.general.client} />
            )}
          </dd>
        </dl>
      </Section>

      <Section n={2} title="Tóm tắt ý chính">
        <StringList editing={editing} items={m.summary} onChange={(summary) => set({ summary })} numbered />
      </Section>

      <Section n={3} title="Thông tin brief">
        <dl className="grid grid-cols-[10rem_1fr] gap-x-4 gap-y-1.5 text-sm">
          {BRIEF.map(({ key, label }) => (
            <Pair key={key} label={label}>
              {editing ? (
                <TextInput value={draft.brief[key]} onChange={(v) => set({ brief: { ...draft.brief, [key]: v } })} />
              ) : (
                <Val v={m.brief[key]} />
              )}
            </Pair>
          ))}
        </dl>
      </Section>

      <Section n={4} title="Hạng mục kỹ thuật & sáng tạo">
        <div className="grid grid-cols-2 gap-3">
          {TECH.map(({ key, label }) => (
            <TechCard
              key={key}
              label={label}
              item={m.technical[key]}
              editing={editing}
              onChange={(item) => set({ technical: { ...draft.technical, [key]: item } })}
            />
          ))}
        </div>
      </Section>

      <Section n={5} title="Run-down / Timeline">
        {editing ? (
          <LinesEditor
            hint="Mỗi dòng: thời gian | nội dung"
            value={draft.rundown.map((r) => `${r.time} | ${r.content}`).join("\n")}
            onChange={(v) => set({ rundown: splitLines(v).map(([time, content]) => ({ time, content: content ?? "" })) })}
          />
        ) : m.rundown.length ? (
          <ul className="space-y-1 text-sm">
            {m.rundown.map((r, i) => (
              <li key={i} className="flex gap-3">
                <span className="w-24 shrink-0 font-mono text-ink-300">{r.time}</span>
                <span>{r.content}</span>
              </li>
            ))}
          </ul>
        ) : (
          <Na />
        )}
      </Section>

      <Section n={6} title="Ngân sách / con số">
        {editing ? (
          <LinesEditor
            hint="Mỗi dòng: hạng mục | số tiền | ghi chú"
            value={draft.budget.map((b) => [b.item, b.amount, b.note].join(" | ")).join("\n")}
            onChange={(v) =>
              set({ budget: splitLines(v).map(([item, amount, note]) => ({ item, amount: amount ?? "", note: note ?? "" })) })
            }
          />
        ) : m.budget.length ? (
          <table className="w-full text-sm">
            <tbody>
              {m.budget.map((b, i) => (
                <tr key={i} className="border-b border-ink-800 last:border-0">
                  <td className="py-1.5 pr-4">{b.item}</td>
                  <td className="py-1.5 pr-4 font-semibold">{b.amount}</td>
                  <td className="py-1.5 text-ink-400">{empty(b.note) ? "" : b.note}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <Na />
        )}
      </Section>

      <Section n={7} title="Đã chốt">
        {editing ? (
          <LinesEditor
            hint="Mỗi dòng: nội dung | mm:ss"
            value={draft.decisions.map((d) => `${d.content} | ${d.source_time}`).join("\n")}
            onChange={(v) =>
              set({ decisions: splitLines(v).map(([content, source_time]) => ({ content, source_time: source_time ?? "" })) })
            }
          />
        ) : m.decisions.length ? (
          <ul className="space-y-1.5 text-sm">
            {m.decisions.map((d, i) => (
              <li key={i} className="flex items-start gap-2">
                <span className="mt-0.5 text-ok">✓</span>
                <span className="flex-1">{d.content}</span>
                <TimeLink ms={parseTime(d.source_time)} onPlay={onPlay} />
              </li>
            ))}
          </ul>
        ) : (
          <Na />
        )}
      </Section>

      <Section n={8} title="Việc cần làm">
        {actionItems.length ? (
          <table className="w-full text-sm">
            <thead className="text-left text-xs uppercase tracking-wider text-ink-400">
              <tr>
                <th className="pb-1.5 font-semibold">Việc</th>
                <th className="pb-1.5 font-semibold">Ai làm</th>
                <th className="pb-1.5 font-semibold">Deadline</th>
                <th className="pb-1.5 font-semibold">Trạng thái</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {actionItems.map((a) => (
                <tr key={a.id} className="border-t border-ink-800">
                  <td className="py-1.5 pr-3">{a.task}</td>
                  <td className="py-1.5 pr-3">{empty(a.owner) ? <Na /> : a.owner}</td>
                  <td className="py-1.5 pr-3">{empty(a.deadline) ? <Na /> : a.deadline}</td>
                  <td className="py-1.5 pr-3 text-ink-300">{ACTION_STATUS_LABEL[a.status]}</td>
                  <td className="py-1.5 text-right">
                    <TimeLink ms={a.sourceMs} onPlay={onPlay} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <Na />
        )}
        <p className="mt-2 text-xs text-ink-400">Sửa ở tab “Việc cần làm”.</p>
      </Section>

      <Section n={9} title="Cần confirm lại với khách">
        <StringList editing={editing} items={m.confirm_with_client} onChange={(confirm_with_client) => set({ confirm_with_client })} />
      </Section>

      <Section n={10} title="Rủi ro / lưu ý">
        <StringList editing={editing} items={m.risks} onChange={(risks) => set({ risks })} />
      </Section>
    </div>
  );
}

function splitLines(v: string): string[][] {
  return v
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean)
    .map((l) => l.split("|").map((x) => x.trim()));
}

function Section({ n, title, children }: { n: number; title: string; children: React.ReactNode }) {
  return (
    <section className="rounded-xl border border-ink-800 bg-ink-900 px-5 py-4">
      <h3 className="mb-3 text-sm font-bold">
        <span className="mr-2 text-ink-400">{n}.</span>
        {title}
      </h3>
      {children}
    </section>
  );
}

function Dt({ children }: { children: React.ReactNode }) {
  return <dt className="text-ink-400">{children}</dt>;
}

function Pair({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <>
      <Dt>{label}</Dt>
      <dd>{children}</dd>
    </>
  );
}

function Na() {
  return <span className="text-sm text-ink-600">{NA}</span>;
}

function Val({ v }: { v: string }) {
  return empty(v) ? <Na /> : <span>{v}</span>;
}

function TextInput({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  return (
    <input
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="w-full rounded border border-ink-700 bg-ink-850 px-2 py-1 text-sm focus:border-ink-400 focus:outline-none"
    />
  );
}

function LinesEditor({ value, onChange, hint }: { value: string; onChange: (v: string) => void; hint?: string }) {
  const rows = Math.max(3, value.split("\n").length + 1);
  return (
    <div>
      <textarea
        value={value}
        onChange={(e) => onChange(e.target.value)}
        rows={rows}
        className="w-full rounded border border-ink-700 bg-ink-850 px-2 py-1.5 text-sm leading-relaxed focus:border-ink-400 focus:outline-none"
      />
      {hint && <p className="mt-1 text-xs text-ink-400">{hint}</p>}
    </div>
  );
}

function StringList({
  items,
  editing,
  onChange,
  numbered,
}: {
  items: string[];
  editing: boolean;
  onChange: (v: string[]) => void;
  numbered?: boolean;
}) {
  if (editing) {
    return (
      <LinesEditor
        hint="Mỗi dòng một ý"
        value={items.join("\n")}
        onChange={(v) => onChange(v.split("\n").map((s) => s.trim()).filter(Boolean))}
      />
    );
  }
  const real = items.filter((i) => !empty(i));
  if (!real.length) return <Na />;
  const Tag = numbered ? "ol" : "ul";
  return (
    <Tag className={`space-y-1 pl-5 text-sm ${numbered ? "list-decimal" : "list-disc"}`}>
      {real.map((s, i) => (
        <li key={i}>{s}</li>
      ))}
    </Tag>
  );
}

function TechCard({
  label,
  item,
  editing,
  onChange,
}: {
  label: string;
  item: TechItem;
  editing: boolean;
  onChange: (i: TechItem) => void;
}) {
  const reqs = item.requirements.filter((r) => !empty(r));
  const nums = item.numbers.filter((r) => !empty(r));
  return (
    <div className="rounded-lg border border-ink-800 px-4 py-3">
      <p className="mb-1.5 text-xs font-semibold uppercase tracking-wider text-ink-400">{label}</p>
      {editing ? (
        <div className="space-y-2">
          <LinesEditor
            hint="Yêu cầu (mỗi dòng một ý)"
            value={item.requirements.join("\n")}
            onChange={(v) => onChange({ ...item, requirements: v.split("\n").map((s) => s.trim()).filter(Boolean) })}
          />
          <LinesEditor
            hint="Con số (mỗi dòng một ý)"
            value={item.numbers.join("\n")}
            onChange={(v) => onChange({ ...item, numbers: v.split("\n").map((s) => s.trim()).filter(Boolean) })}
          />
        </div>
      ) : reqs.length || nums.length ? (
        <>
          {reqs.length > 0 && (
            <ul className="list-disc space-y-0.5 pl-4 text-sm">
              {reqs.map((r, i) => (
                <li key={i}>{r}</li>
              ))}
            </ul>
          )}
          {nums.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-1.5">
              {nums.map((n, i) => (
                <span key={i} className="rounded bg-ink-800 px-2 py-0.5 font-mono text-xs text-zinc-200">
                  {n}
                </span>
              ))}
            </div>
          )}
        </>
      ) : (
        <Na />
      )}
    </div>
  );
}
