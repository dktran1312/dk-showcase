import type { SttStatus } from "../lib/types";

export function SttBanner({ status }: { status: SttStatus | null }) {
  if (!status || status.state === "ready") return null;
  if (status.state === "loading") {
    return (
      <div className="border-b border-ink-800 bg-ink-900 px-8 py-2 text-sm text-ink-300">
        <span className="rec-dot mr-2 inline-block h-2 w-2 rounded-full bg-white" />
        Đang nạp model nhận giọng nói ({status.model})… phụ đề sẽ hiện sau vài giây. Âm thanh vẫn đang được ghi.
      </div>
    );
  }
  const msg = status.state === "unavailable" ? status.reason : status.message;
  return (
    <div className="border-b border-warn/30 bg-warn/10 px-8 py-2 text-sm text-amber-200">
      ⚠️ Không có phụ đề trực tiếp: {msg}
    </div>
  );
}
