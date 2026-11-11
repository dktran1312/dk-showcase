/** Wordmark "DK TRAN" — chữ trắng, đậm. */
export function Logo({ size = "sm" }: { size?: "sm" | "lg" }) {
  if (size === "lg") {
    return (
      <div className="flex flex-col items-center gap-3">
        <div className="text-7xl font-black tracking-[0.08em] text-white leading-none">DK TRAN</div>
        <div className="flex items-center gap-3 text-sm font-medium uppercase tracking-[0.5em] text-ink-400">
          <span className="h-px w-10 bg-ink-600" />
          Meeting
          <span className="h-px w-10 bg-ink-600" />
        </div>
      </div>
    );
  }
  return (
    <div className="flex items-baseline gap-2 leading-none">
      <span className="text-[15px] font-black tracking-[0.08em] text-white">DK TRAN</span>
      <span className="text-[10px] font-semibold uppercase tracking-[0.3em] text-ink-400">Meeting</span>
    </div>
  );
}
