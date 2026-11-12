import { useEffect, useRef, useState } from "react";
import { api, errorText } from "../lib/api";
import { LANGUAGE_LABEL, type InputDevice, type Language, type Settings, type SystemInfo } from "../lib/types";
import { Button } from "../components/Button";
import { Toggle } from "../components/Toggle";
import { WhisperModels } from "../components/WhisperModels";
import { OllamaPanel } from "../components/OllamaPanel";

export function SettingsPage({ info, onSystemCheck }: { info: SystemInfo; onSystemCheck: () => void }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [devices, setDevices] = useState<InputDevice[]>([]);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string>();

  const latest = useRef<Settings | null>(null);
  latest.current = settings;

  useEffect(() => {
    api.getSettings().then(setSettings).catch((e) => setError(errorText(e)));
    api.listInputDevices().then(setDevices).catch((e) => setError(errorText(e)));
  }, []);

  const update = async (patch: Partial<Settings>) => {
    if (!latest.current) return;
    const next = { ...latest.current, ...patch };
    latest.current = next;
    setSettings(next);
    try {
      await api.saveSettings(next);
      setSaved(true);
      setTimeout(() => setSaved(false), 1200);
    } catch (e) {
      setError(errorText(e));
    }
  };

  return (
    <div className="h-full overflow-y-auto px-10 py-10">
      <div className="mx-auto max-w-3xl">
        <div className="flex items-baseline justify-between gap-4">
          <h1 className="text-3xl font-extrabold tracking-tight">Cài đặt</h1>
          <span className={`flex-1 text-sm text-ok transition-opacity ${saved ? "opacity-100" : "opacity-0"}`}>Đã lưu ✓</span>
          <Button variant="ghost" onClick={onSystemCheck}>
            Kiểm tra hệ thống
          </Button>
        </div>
        {error && <p className="mt-4 text-sm text-rec">{error}</p>}

        {settings && (
          <div className="mt-8 space-y-4">
            <Card title="Âm thanh">
              <Row label="Micro mặc định">
                <select
                  value={settings.micDevice ?? ""}
                  onChange={(e) => update({ micDevice: e.target.value || null })}
                  className={selectCls}
                >
                  <option value="">Micro mặc định của hệ thống</option>
                  {devices.map((d) => (
                    <option key={d.name} value={d.name}>
                      {d.name}
                    </option>
                  ))}
                </select>
              </Row>
              <Row label="Ngôn ngữ mặc định">
                <select
                  value={settings.language}
                  onChange={(e) => update({ language: e.target.value as Language })}
                  className={selectCls}
                >
                  {(Object.keys(LANGUAGE_LABEL) as Language[]).map((l) => (
                    <option key={l} value={l}>
                      {LANGUAGE_LABEL[l]}
                    </option>
                  ))}
                </select>
              </Row>
            </Card>

            <Card title="Nhận giọng nói (phụ đề trực tiếp)">
              <Row label="Bật phụ đề trực tiếp">
                <Toggle checked={settings.sttEnabled} onChange={(v) => update({ sttEnabled: v })} label="Bật phụ đề" />
              </Row>
              <WhisperModels selected={settings.whisperModel} onSelect={(id) => update({ whisperModel: id })} />
            </Card>

            <Card title="Dịch & biên bản (Ollama)">
              <Row label="Dịch trực tiếp sang tiếng Việt">
                <Toggle
                  checked={settings.translateEnabled}
                  onChange={(v) => update({ translateEnabled: v })}
                  label="Bật dịch trực tiếp"
                />
              </Row>
              <OllamaPanel model={settings.ollamaModel} onSelect={(m) => update({ ollamaModel: m })} />
            </Card>

            <Card title="Dữ liệu">
              <Row label="Thư mục dữ liệu">
                <div className="flex min-w-0 items-center gap-3">
                  <code title={info.dataDir} className="selectable min-w-0 truncate rounded bg-ink-850 px-2 py-1 text-xs text-ink-300">{info.dataDir}</code>
                  <Button variant="ghost" onClick={() => api.revealInFolder(info.dbPath).catch((e) => setError(errorText(e)))}>
                    Mở
                  </Button>
                </div>
              </Row>
              <Row label="Giao diện">
                <span className="text-sm text-ink-300">Tối</span>
              </Row>
            </Card>

            <Card title="Sắp có">
              <Soon label="Sửa từ điển thuật ngữ (đang dùng sẵn 57 thuật ngữ ngành sự kiện)" milestone="M5" />
              <Soon label="Thu âm thanh app họp (Zalo, Meet, Messenger)" milestone="M2" />
            </Card>

            <p className="pt-4 text-xs text-ink-600">
              DK Meeting v{info.version} · {info.os}/{info.arch}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

const selectCls =
  "h-10 w-80 rounded-lg border border-ink-700 bg-ink-900 px-3 text-sm focus:border-ink-400 focus:outline-none";

function Card({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="rounded-xl border border-ink-800 bg-ink-900">
      <h2 className="border-b border-ink-800 px-5 py-3 text-xs font-semibold uppercase tracking-wider text-ink-400">{title}</h2>
      <div className="divide-y divide-ink-800">{children}</div>
    </section>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 px-5 py-3.5">
      <span className="shrink-0 text-sm font-medium">{label}</span>
      <div className="flex min-w-0 justify-end">{children}</div>
    </div>
  );
}

function Soon({ label, milestone }: { label: string; milestone: string }) {
  return (
    <div className="flex items-center justify-between px-5 py-3 text-sm text-ink-400">
      {label}
      <span className="rounded bg-ink-800 px-1.5 py-0.5 text-[10px] font-semibold">{milestone}</span>
    </div>
  );
}
