export function ConsentBanner() {
  return (
    <div className="flex items-start gap-3 rounded-xl border border-warn/30 bg-warn/10 px-4 py-3 text-sm text-amber-200">
      <span aria-hidden className="mt-0.5 text-base">⚠️</span>
      <p>
        <strong className="font-semibold text-amber-100">
          Hãy thông báo cho mọi người là cuộc họp đang được ghi âm.
        </strong>{" "}
        File ghi âm chỉ lưu trên máy này.
      </p>
    </div>
  );
}
