import { useEffect, useState } from "react";
import { api, errorText, events, subscribe } from "../lib/api";
import { formatBytes } from "../lib/format";
import type { DownloadProgress, WhisperModel } from "../lib/types";
import { Button } from "./Button";
import { ProgressBar } from "./ProgressBar";
import { askConfirm } from "./ConfirmDialog";

interface Props {
  selected: string;
  recommended?: string;
  onSelect: (id: string) => void;
  onChange?: (models: WhisperModel[]) => void;
}

/** Danh sách model whisper: tải / huỷ / xoá / chọn dùng. */
export function WhisperModels({ selected, recommended, onSelect, onChange }: Props) {
  const [models, setModels] = useState<WhisperModel[]>([]);
  const [progress, setProgress] = useState<Record<string, DownloadProgress>>({});
  const [error, setError] = useState<string>();

  const refresh = () =>
    api
      .listWhisperModels()
      .then((m) => {
        setModels(m);
        onChange?.(m);
      })
      .catch((e) => setError(errorText(e)));

  useEffect(() => {
    refresh();
    return subscribe(
      events.onModelProgress((p) => {
        setProgress((x) => {
          if (p.done) {
            const { [p.id]: _, ...rest } = x;
            return rest;
          }
          return { ...x, [p.id]: p };
        });
        if (p.done) {
          if (p.error && p.error !== "Đã huỷ tải") setError(p.error);
          refresh();
        }
      }),
    );
  }, []);

  const download = (m: WhisperModel) => {
    setError(undefined);
    setProgress((x) => ({ ...x, [m.id]: { id: m.id, downloaded: 0, total: m.sizeMb * 1024 * 1024, done: false, error: null } }));
    api
      .downloadWhisperModel(m.id)
      .then(() => {
        if (!models.some((x) => x.installed && x.id === selected)) onSelect(m.id);
      })
      .catch(() => {});
  };

  const remove = async (m: WhisperModel) => {
    const ok = await askConfirm({
      title: `Xoá model ${m.label}?`,
      message: `Giải phóng khoảng ${m.sizeMb} MB. Có thể tải lại bất cứ lúc nào.`,
      confirmLabel: "Xoá",
      danger: true,
    });
    if (!ok) return;
    api.deleteWhisperModel(m.id).then(refresh).catch((e) => setError(errorText(e)));
  };

  return (
    <div className="divide-y divide-ink-800">
      {models.map((m) => {
        const p = progress[m.id];
        const isSel = m.id === selected;
        return (
          <div key={m.id} className="flex items-center gap-4 px-5 py-3.5">
            <button
              type="button"
              disabled={!m.installed}
              onClick={() => onSelect(m.id)}
              aria-label={`Dùng model ${m.label}`}
              className={`h-4 w-4 shrink-0 rounded-full border-2 disabled:opacity-30 ${
                isSel && m.installed ? "border-white bg-white shadow-[inset_0_0_0_3px_var(--color-ink-900)]" : "border-ink-600"
              }`}
            />
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium">
                {m.label}
                <span className="ml-2 text-xs text-ink-400">{m.sizeMb} MB</span>
                {m.id === recommended && (
                  <span className="ml-2 rounded bg-ok/15 px-1.5 py-0.5 text-[10px] font-semibold text-ok">Gợi ý cho máy này</span>
                )}
                {isSel && m.installed && (
                  <span className="ml-2 rounded bg-ink-700 px-1.5 py-0.5 text-[10px] font-semibold">Đang dùng</span>
                )}
              </p>
              <p className="text-xs text-ink-400">{m.description}</p>
              {p && (
                <div className="mt-2">
                  <ProgressBar
                    value={p.downloaded}
                    total={p.total}
                    label={`${formatBytes(p.downloaded)} / ${p.total ? formatBytes(p.total) : "?"}`}
                  />
                </div>
              )}
            </div>
            {p ? (
              <Button variant="ghost" onClick={() => api.cancelDownload(m.id)}>
                Huỷ
              </Button>
            ) : m.installed ? (
              <Button variant="ghost" onClick={() => remove(m)}>
                Xoá
              </Button>
            ) : (
              <Button onClick={() => download(m)}>Tải về</Button>
            )}
          </div>
        );
      })}
      {error && <p className="px-5 py-3 text-sm text-rec">{error}</p>}
    </div>
  );
}
