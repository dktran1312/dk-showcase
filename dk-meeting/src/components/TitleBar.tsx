import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Logo } from "./Logo";
import { askConfirm } from "./ConfirmDialog";
import { formatDuration } from "../lib/format";

export type NavKey = "home" | "settings";

interface Props {
  active: NavKey | null;
  onNavigate: (key: NavKey) => void;
  recording: { elapsedMs: number; paused: boolean } | null;
  onOpenLive: () => void;
}

export function TitleBar({ active, onNavigate, recording, onOpenLive }: Props) {
  const win = getCurrentWindow();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    win.isMaximized().then(setMaximized).catch(() => {});
    const un = win.onResized(() => win.isMaximized().then(setMaximized).catch(() => {}));
    return () => {
      un.then((f) => f());
    };
  }, [win]);

  const nav = (key: NavKey, label: string) => (
    <button
      onClick={() => onNavigate(key)}
      className={`rounded-md px-3 py-1.5 text-[13px] font-medium transition-colors ${
        active === key ? "bg-ink-800 text-white" : "text-ink-400 hover:text-white"
      }`}
    >
      {label}
    </button>
  );

  return (
    <header
      data-tauri-drag-region
      className="flex h-11 shrink-0 items-center gap-4 border-b border-ink-800 bg-ink-950 pl-4"
    >
      <div data-tauri-drag-region className="pointer-events-none pr-2">
        <Logo />
      </div>
      <nav className="flex items-center gap-1">
        {nav("home", "Cuộc họp")}
        {nav("settings", "Cài đặt")}
      </nav>

      <div data-tauri-drag-region className="flex h-full flex-1 items-center justify-center">
        {recording && (
          <button
            onClick={onOpenLive}
            className="flex items-center gap-2 rounded-full border border-rec/40 bg-rec-soft px-3 py-1 text-xs font-semibold text-red-200 hover:border-rec"
          >
            <span className={`h-2 w-2 rounded-full bg-rec ${recording.paused ? "" : "rec-dot"}`} />
            {recording.paused ? "Tạm dừng" : "Đang ghi"} · {formatDuration(recording.elapsedMs)}
          </button>
        )}
      </div>

      <div className="flex h-full">
        <WinButton label="Thu nhỏ" onClick={() => win.minimize()}>
          <path d="M1 5.5h9" />
        </WinButton>
        <WinButton label={maximized ? "Khôi phục" : "Phóng to"} onClick={() => win.toggleMaximize()}>
          {maximized ? (
            <>
              <rect x="1" y="3" width="7" height="7" />
              <path d="M3 3V1h7v7H8" />
            </>
          ) : (
            <rect x="1" y="1" width="9" height="9" />
          )}
        </WinButton>
        <WinButton
          label="Đóng"
          danger
          onClick={async () => {
            if (
              recording &&
              !(await askConfirm({
                title: "Đóng DK Meeting?",
                message: "Cuộc họp đang được ghi âm. Đóng app sẽ dừng ghi và lưu lại phần đã ghi.",
                confirmLabel: "Dừng & đóng",
                danger: true,
              }))
            )
              return;
            win.close();
          }}
        >
          <path d="M1 1l9 9M10 1l-9 9" />
        </WinButton>
      </div>
    </header>
  );
}

function WinButton({
  children,
  label,
  onClick,
  danger,
}: {
  children: React.ReactNode;
  label: string;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      aria-label={label}
      title={label}
      onClick={onClick}
      className={`flex h-full w-12 items-center justify-center text-ink-300 transition-colors ${
        danger ? "hover:bg-rec hover:text-white" : "hover:bg-ink-800 hover:text-white"
      }`}
    >
      <svg width="11" height="11" viewBox="0 0 11 11" fill="none" stroke="currentColor" strokeWidth="1.1">
        {children}
      </svg>
    </button>
  );
}
