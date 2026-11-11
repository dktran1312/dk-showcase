import { Logo } from "./Logo";

export function Splash({ message, error }: { message?: string; error?: string }) {
  return (
    <div data-tauri-drag-region className="flex h-full flex-col items-center justify-center bg-ink-950">
      <div className="splash-in">
        <Logo size="lg" />
      </div>
      <div className="mt-12 h-6 text-sm text-ink-400">
        {error ? <span className="text-rec">{error}</span> : message}
      </div>
    </div>
  );
}
