import { useEffect, useState } from "react";
import { api, errorText, events, subscribe } from "../lib/api";
import { formatBytes } from "../lib/format";
import type { OllamaStatus } from "../lib/types";
import { Button } from "./Button";
import { ProgressBar } from "./ProgressBar";

const SUGGESTED = ["qwen3:8b", "qwen3:14b", "gemma3:12b", "qwen3:4b"];

interface Props {
  model: string;
  onSelect: (model: string) => void;
  onStatus?: (s: OllamaStatus) => void;
}

/** Trạng thái Ollama + chọn / tải model LLM. */
export function OllamaPanel({ model, onSelect, onStatus }: Props) {
  const [status, setStatus] = useState<OllamaStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [pullName, setPullName] = useState("");
  const [pull, setPull] = useState<{ model: string; status: string; completed: number; total: number } | null>(null);
  const [error, setError] = useState<string>();

  const refresh = () => {
    setChecking(true);
    api
      .ollamaStatus()
      .then((s) => {
        setStatus(s);
        onStatus?.(s);
      })
      .catch((e) => setError(errorText(e)))
      .finally(() => setChecking(false));
  };

  useEffect(() => {
    refresh();
    return subscribe(events.onOllamaPull(setPull));
  }, []);

  useEffect(() => {
    // model đã cấu hình có thể vừa đổi → kiểm tra lại
    if (status) refresh();
  }, [model]);

  const doPull = (name: string) => {
    setError(undefined);
    setPull({ model: name, status: "Bắt đầu tải…", completed: 0, total: 0 });
    api
      .ollamaPull(name)
      .then(() => {
        onSelect(name);
        setPullName("");
      })
      .catch((e) => setError(errorText(e)))
      .finally(() => {
        setPull(null);
        refresh();
      });
  };

  if (!status) return <p className="px-5 py-4 text-sm text-ink-400">Đang kiểm tra Ollama…</p>;

  if (!status.running) {
    return (
      <div className="space-y-3 px-5 py-4 text-sm">
        <p className="text-amber-200">⚠️ Chưa kết nối được Ollama tại {status.url}.</p>
        <ol className="list-decimal space-y-1 pl-5 text-ink-300">
          <li>
            Tải và cài Ollama tại <span className="selectable font-mono">ollama.com/download</span>
          </li>
          <li>Mở ứng dụng Ollama (biểu tượng 🦙 cạnh đồng hồ Windows)</li>
          <li>Bấm “Kiểm tra lại”</li>
        </ol>
        <Button onClick={refresh} disabled={checking}>
          {checking ? "Đang kiểm tra…" : "Kiểm tra lại"}
        </Button>
      </div>
    );
  }

  const installed = status.models.map((m) => m.name);
  const missingSuggested = SUGGESTED.filter((s) => !installed.includes(s));

  return (
    <div className="divide-y divide-ink-800">
      <div className="flex items-center justify-between gap-4 px-5 py-3.5">
        <span className="text-sm">
          <span className="text-ok">●</span> Ollama {status.version} đang chạy
        </span>
        <Button variant="ghost" onClick={refresh} disabled={checking}>
          Kiểm tra lại
        </Button>
      </div>

      <div className="flex items-center justify-between gap-6 px-5 py-3.5">
        <span className="shrink-0 text-sm font-medium">Model dùng để dịch & tóm tắt</span>
        <select
          value={installed.includes(model) ? model : ""}
          onChange={(e) => onSelect(e.target.value)}
          className="h-10 w-72 rounded-lg border border-ink-700 bg-ink-900 px-3 text-sm focus:border-ink-400 focus:outline-none"
        >
          {!installed.includes(model) && <option value="">{model} (chưa tải)</option>}
          {status.models.map((m) => (
            <option key={m.name} value={m.name}>
              {m.name} · {formatBytes(m.size)}
            </option>
          ))}
        </select>
      </div>

      {!status.hasModel && !pull && (
        <div className="flex items-center justify-between gap-4 bg-warn/5 px-5 py-3.5 text-sm">
          <span className="text-amber-200">Chưa có model “{model}” trong Ollama.</span>
          <Button onClick={() => doPull(model)}>Tải {model}</Button>
        </div>
      )}

      {pull && (
        <div className="px-5 py-3.5">
          <p className="mb-2 text-sm">Đang tải {pull.model}…</p>
          <ProgressBar
            value={pull.completed}
            total={pull.total}
            label={pull.total ? `${formatBytes(pull.completed)} / ${formatBytes(pull.total)}` : pull.status}
          />
        </div>
      )}

      <div className="flex items-center gap-3 px-5 py-3.5">
        <input
          value={pullName}
          onChange={(e) => setPullName(e.target.value)}
          placeholder="Tải model khác, vd. gemma3:12b"
          list="ollama-suggest"
          className="h-10 flex-1 rounded-lg border border-ink-700 bg-ink-900 px-3 text-sm placeholder:text-ink-400 focus:border-ink-400 focus:outline-none"
        />
        <datalist id="ollama-suggest">
          {missingSuggested.map((s) => (
            <option key={s} value={s} />
          ))}
        </datalist>
        <Button onClick={() => doPull(pullName)} disabled={!pullName.trim() || !!pull}>
          Tải
        </Button>
      </div>
      {error && <p className="px-5 py-3 text-sm text-rec">{error}</p>}
    </div>
  );
}
