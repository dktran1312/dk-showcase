import { useEffect, useRef, useState } from "react";
import { ACTION_STATUS_LABEL, type ActionItem, type ActionStatus } from "../lib/types";
import { Button } from "./Button";
import { TimeLink } from "./TimeLink";

interface Props {
  items: ActionItem[];
  onSave: (items: ActionItem[]) => Promise<void>;
  onPlay: (ms: number) => void;
}

const STATUS_STYLE: Record<ActionStatus, string> = {
  todo: "border-ink-600 text-ink-300",
  doing: "border-sky-400/50 text-sky-300",
  done: "border-ok/50 text-ok",
};

/** Bảng việc cần làm: sửa trực tiếp, tự lưu. */
export function ActionItemsTable({ items, onSave, onPlay }: Props) {
  const [rows, setRows] = useState<ActionItem[]>(items);
  const [saved, setSaved] = useState(true);
  const timer = useRef<number>();
  useEffect(() => setRows(items), [items]);

  const commit = (next: ActionItem[], immediate = false) => {
    setRows(next);
    setSaved(false);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(
      () => onSave(next.filter((r) => r.task.trim())).then(() => setSaved(true)),
      immediate ? 0 : 800,
    );
  };

  const update = (i: number, patch: Partial<ActionItem>, immediate = false) =>
    commit(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)), immediate);

  const done = rows.filter((r) => r.status === "done").length;

  return (
    <div className="selectable">
      <div className="mb-3 flex items-center justify-between">
        <p className="text-sm text-ink-400">
          {rows.length} việc · {done} xong
          <span className={`ml-3 text-xs ${saved ? "text-ink-600" : "text-amber-300"}`}>{saved ? "Đã lưu" : "Đang lưu…"}</span>
        </p>
        <Button
          onClick={() =>
            setRows((r) => [...r, { id: 0, task: "", owner: "", deadline: "", status: "todo", sourceMs: null }])
          }
        >
          + Thêm việc
        </Button>
      </div>
      {rows.length === 0 ? (
        <p className="rounded-xl border border-dashed border-ink-700 px-6 py-12 text-center text-sm text-ink-400">
          Chưa có việc cần làm. Bấm “Tạo biên bản” để AI tự liệt kê, hoặc “+ Thêm việc”.
        </p>
      ) : (
        <table className="w-full table-fixed text-sm">
          <thead className="text-left text-xs uppercase tracking-wider text-ink-400">
            <tr>
              <th className="w-[44%] pb-2 font-semibold">Việc</th>
              <th className="w-[17%] pb-2 font-semibold">Người phụ trách</th>
              <th className="w-[15%] pb-2 font-semibold">Deadline</th>
              <th className="w-[13%] pb-2 font-semibold">Trạng thái</th>
              <th className="w-[11%]" />
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={`${r.id}-${i}`} className={`border-t border-ink-800 ${r.status === "done" ? "opacity-60" : ""}`}>
                <td className="py-1 pr-2">
                  <Cell value={r.task} placeholder="Nội dung việc…" onChange={(task) => update(i, { task })} strike={r.status === "done"} />
                </td>
                <td className="py-1 pr-2">
                  <Cell value={r.owner} placeholder="Ai làm" onChange={(owner) => update(i, { owner })} />
                </td>
                <td className="py-1 pr-2">
                  <Cell value={r.deadline} placeholder="Hạn" onChange={(deadline) => update(i, { deadline })} />
                </td>
                <td className="py-1 pr-2">
                  <select
                    value={r.status}
                    onChange={(e) => update(i, { status: e.target.value as ActionStatus }, true)}
                    className={`h-8 w-full rounded-md border bg-transparent px-2 text-xs font-semibold ${STATUS_STYLE[r.status]}`}
                  >
                    {(Object.keys(ACTION_STATUS_LABEL) as ActionStatus[]).map((s) => (
                      <option key={s} value={s}>
                        {ACTION_STATUS_LABEL[s]}
                      </option>
                    ))}
                  </select>
                </td>
                <td className="py-1 text-right">
                  <div className="flex items-center justify-end">
                    <TimeLink ms={r.sourceMs} onPlay={onPlay} />
                    <button
                      onClick={() => commit(rows.filter((_, j) => j !== i), true)}
                      title="Xoá việc"
                      className="rounded px-2 py-1 text-ink-600 hover:bg-ink-800 hover:text-rec"
                    >
                      ✕
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

function Cell({
  value,
  onChange,
  placeholder,
  strike,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  strike?: boolean;
}) {
  return (
    <input
      value={value === "Chưa đề cập" ? "" : value}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
      className={`h-8 w-full rounded-md border border-transparent bg-transparent px-2 placeholder:text-ink-600 hover:border-ink-700 focus:border-ink-400 focus:bg-ink-850 focus:outline-none ${
        strike ? "line-through" : ""
      }`}
    />
  );
}
