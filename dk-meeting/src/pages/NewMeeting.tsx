import { useEffect, useState } from "react";
import { api, errorText } from "../lib/api";
import { defaultMeetingTitle } from "../lib/format";
import { LANGUAGE_LABEL, type InputDevice, type Language, type Meeting } from "../lib/types";
import { Button } from "../components/Button";
import { ConsentBanner } from "../components/ConsentBanner";

type SourceMode = "mic" | "app" | "both";

interface Props {
  onCancel: () => void;
  onStarted: (m: Meeting) => void;
}

export function NewMeeting({ onCancel, onStarted }: Props) {
  const [title, setTitle] = useState(defaultMeetingTitle());
  const [project, setProject] = useState("");
  const [client, setClient] = useState("");
  const [language, setLanguage] = useState<Language>("auto");
  const [mode, setMode] = useState<SourceMode>("mic");
  const [devices, setDevices] = useState<InputDevice[]>([]);
  const [mic, setMic] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    Promise.all([api.listInputDevices(), api.getSettings()])
      .then(([devs, s]) => {
        setDevices(devs);
        setLanguage(s.language);
        if (s.micDevice && devs.some((d) => d.name === s.micDevice)) setMic(s.micDevice);
      })
      .catch((e) => setError(errorText(e)));
  }, []);

  const start = async () => {
    setBusy(true);
    setError(undefined);
    try {
      const m = await api.startMeeting({ title, project, client, language, micDevice: mic || null });
      onStarted(m);
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  return (
    <div className="h-full overflow-y-auto px-10 py-10">
      <div className="mx-auto max-w-2xl">
        <button onClick={onCancel} className="mb-6 text-sm text-ink-400 hover:text-white">
          ← Cuộc họp
        </button>
        <h1 className="text-3xl font-extrabold tracking-tight">Bắt đầu họp</h1>

        <div className="mt-8 space-y-6">
          <Field label="Tên cuộc họp">
            <input autoFocus value={title} onChange={(e) => setTitle(e.target.value)} className={inputCls} />
          </Field>
          <div className="grid grid-cols-2 gap-4">
            <Field label="Dự án">
              <input value={project} onChange={(e) => setProject(e.target.value)} placeholder="VD: Concert Tết 2027" className={inputCls} />
            </Field>
            <Field label="Khách hàng / nhãn hàng / nghệ sĩ">
              <input value={client} onChange={(e) => setClient(e.target.value)} placeholder="VD: Vinamilk" className={inputCls} />
            </Field>
          </div>

          <Field label="Nguồn thu">
            <div className="grid grid-cols-3 gap-3">
              <SourceCard active={mode === "mic"} onClick={() => setMode("mic")} title="Micro" desc="Họp trực tiếp" />
              <SourceCard disabled title="App họp" desc="Meet · Zalo · Messenger" badge="M2" />
              <SourceCard disabled title="Cả hai" desc="Micro + app họp" badge="M2" />
            </div>
          </Field>

          {mode === "mic" && (
            <Field label="Micro">
              <select value={mic} onChange={(e) => setMic(e.target.value)} className={inputCls}>
                <option value="">Micro mặc định của hệ thống</option>
                {devices.map((d) => (
                  <option key={d.name} value={d.name}>
                    {d.name}
                    {d.isDefault ? " (mặc định)" : ""}
                  </option>
                ))}
              </select>
            </Field>
          )}

          <Field label="Ngôn ngữ cuộc họp">
            <div className="flex gap-2">
              {(Object.keys(LANGUAGE_LABEL) as Language[]).map((l) => (
                <button
                  key={l}
                  onClick={() => setLanguage(l)}
                  className={`rounded-lg border px-4 py-2 text-sm font-medium ${
                    language === l ? "border-white bg-white text-ink-950" : "border-ink-700 text-ink-300 hover:border-ink-400"
                  }`}
                >
                  {LANGUAGE_LABEL[l]}
                </button>
              ))}
            </div>
          </Field>

          <ConsentBanner />

          {error && <p className="rounded-lg bg-rec-soft px-4 py-3 text-sm text-red-200">{error}</p>}

          <div className="flex justify-end gap-3 pt-2">
            <Button variant="ghost" onClick={onCancel}>
              Huỷ
            </Button>
            <Button variant="record" onClick={start} disabled={busy || !title.trim()} className="h-12 rounded-xl px-6">
              <span className="h-2.5 w-2.5 rounded-full bg-white" />
              {busy ? "Đang mở micro…" : "Bắt đầu ghi âm"}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}

const inputCls =
  "h-11 w-full rounded-lg border border-ink-700 bg-ink-900 px-3 text-sm placeholder:text-ink-400 focus:border-ink-400 focus:outline-none";

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <span className="mb-2 block text-xs font-semibold uppercase tracking-wider text-ink-400">{label}</span>
      {children}
    </label>
  );
}

function SourceCard({
  title,
  desc,
  active,
  disabled,
  badge,
  onClick,
}: {
  title: string;
  desc: string;
  active?: boolean;
  disabled?: boolean;
  badge?: string;
  onClick?: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={`relative rounded-xl border p-4 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${
        active ? "border-white bg-ink-850" : "border-ink-700 hover:border-ink-400"
      }`}
    >
      {badge && (
        <span className="absolute right-3 top-3 rounded bg-ink-700 px-1.5 py-0.5 text-[10px] font-semibold text-ink-300">
          {badge}
        </span>
      )}
      <span className="block font-semibold">{title}</span>
      <span className="mt-0.5 block text-xs text-ink-400">{desc}</span>
    </button>
  );
}
