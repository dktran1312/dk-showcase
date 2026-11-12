import { useEffect, useRef, useState } from "react";
import { api, errorText } from "../lib/api";
import type { Settings, SystemCheck } from "../lib/types";
import { Button } from "../components/Button";
import { WhisperModels } from "../components/WhisperModels";
import { OllamaPanel } from "../components/OllamaPanel";

/** Màn hình "Kiểm tra hệ thống": hiện lần đầu mở app, mở lại được từ Cài đặt. */
export function SystemCheckPage({ onDone, firstRun }: { onDone: () => void; firstRun: boolean }) {
  const [check, setCheck] = useState<SystemCheck | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string>();
  const [whisperOk, setWhisperOk] = useState(false);
  const [ollamaOk, setOllamaOk] = useState(false);
  const latest = useRef<Settings | null>(null);
  latest.current = settings;

  useEffect(() => {
    api
      .systemCheck()
      .then((c) => {
        setCheck(c);
        setSettings(c.settings);
        setWhisperOk(c.whisperModels.some((m) => m.installed));
        setOllamaOk(c.ollama.running && c.ollama.hasModel);
      })
      .catch((e) => setError(errorText(e)));
  }, []);

  const update = (patch: Partial<Settings>) => {
    if (!latest.current) return;
    const next = { ...latest.current, ...patch };
    latest.current = next;
    setSettings(next);
    api.saveSettings(next).catch((e) => setError(errorText(e)));
  };

  const finish = () => {
    if (latest.current && !latest.current.onboarded) update({ onboarded: true });
    onDone();
  };

  if (error) return <p className="p-10 text-rec">{error}</p>;
  if (!check || !settings) {
    return <p className="p-10 text-sm text-ink-400">Đang kiểm tra cấu hình máy…</p>;
  }

  const hw = check.hardware;
  const gpu = hw.gpus[0];
  const nvidiaNoCuda = hw.gpus.some((g) => g.nvidia) && check.sttBackend === "CPU";
  const gpuOk = check.sttBackend !== "CPU";

  return (
    <div className="h-full overflow-y-auto px-10 py-10">
      <div className="mx-auto max-w-3xl">
        <h1 className="text-3xl font-extrabold tracking-tight">Kiểm tra hệ thống</h1>
        <p className="mt-1 text-sm text-ink-400">
          DK Meeting chạy AI hoàn toàn trên máy này. Hãy chuẩn bị 2 model bên dưới (chỉ cần làm một lần).
        </p>

        <section className="mt-8 grid grid-cols-2 gap-3">
          <Stat
            label="Card đồ hoạ"
            value={gpu ? gpu.name : "Không tìm thấy"}
            sub={gpu?.vramGb ? `${gpu.vramGb.toFixed(0)} GB VRAM` : undefined}
            ok={gpuOk ? true : gpu ? undefined : false}
          />
          <Stat
            label="Nhận giọng nói chạy bằng"
            value={check.sttBackend === "CPU" ? "CPU" : `GPU (${check.sttBackend})`}
            sub={gpuOk ? "Nhanh, phụ đề gần như tức thì" : "Chậm hơn; nên dùng model nhỏ"}
            ok={gpuOk}
          />
          <Stat label="RAM" value={`${hw.ramGb.toFixed(0)} GB`} ok={hw.ramGb >= 16 ? true : undefined} />
          <Stat label="CPU" value={hw.cpu || "—"} sub={`${hw.cores} luồng · ${hw.os}`} />
        </section>

        {nvidiaNoCuda && (
          <p className="mt-4 rounded-xl border border-warn/30 bg-warn/10 px-4 py-3 text-sm text-amber-200">
            Máy có card NVIDIA nhưng bản app này chưa được build kèm CUDA nên đang nhận giọng nói bằng CPU. Xem
            README mục “Bật GPU (CUDA)” để chạy bằng lệnh <code className="selectable font-mono">npm run dev:cuda</code>.
          </p>
        )}

        <Step n={1} title="Model nhận giọng nói (whisper)" done={whisperOk}>
          <WhisperModels
            onChange={(ms) => setWhisperOk(ms.some((m) => m.installed))}
            selected={settings.whisperModel}
            recommended={check.recommendedModel}
            onSelect={(id) => update({ whisperModel: id })}
          />
        </Step>

        <Step n={2} title="Ollama — dịch & viết biên bản" done={ollamaOk}>
          <OllamaPanel
            model={settings.ollamaModel}
            onSelect={(m) => update({ ollamaModel: m })}
            onStatus={(s) => setOllamaOk(s.running && s.hasModel)}
          />
        </Step>

        <div className="mt-8 flex items-center justify-end gap-3">
          <span className="text-sm text-ink-400">Thiếu model vẫn ghi âm được; bổ sung sau trong Cài đặt.</span>
          <Button variant="primary" onClick={finish}>
            {firstRun ? "Vào app" : "Xong"}
          </Button>
        </div>
      </div>
    </div>
  );
}

function Stat({ label, value, sub, ok }: { label: string; value: string; sub?: string; ok?: boolean }) {
  return (
    <div className="rounded-xl border border-ink-800 bg-ink-900 px-4 py-3">
      <p className="text-xs font-semibold uppercase tracking-wider text-ink-400">{label}</p>
      <p className="mt-1 truncate font-semibold" title={value}>
        {ok === true && <span className="mr-1.5 text-ok">✓</span>}
        {ok === false && <span className="mr-1.5 text-rec">✕</span>}
        {value}
      </p>
      {sub && <p className="truncate text-xs text-ink-400">{sub}</p>}
    </div>
  );
}

function Step({ n, title, done, children }: { n: number; title: string; done: boolean; children: React.ReactNode }) {
  return (
    <section className="mt-6 rounded-xl border border-ink-800 bg-ink-900">
      <h2 className="flex items-center gap-3 border-b border-ink-800 px-5 py-3 text-sm font-semibold">
        <span
          className={`flex h-6 w-6 items-center justify-center rounded-full text-xs ${
            done ? "bg-ok text-ink-950" : "bg-ink-700 text-white"
          }`}
        >
          {done ? "✓" : n}
        </span>
        {title}
      </h2>
      {children}
    </section>
  );
}
