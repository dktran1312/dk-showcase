import { useEffect, useRef, useState } from "react";
import { Button } from "./Button";

interface Request {
  title: string;
  message?: string;
  confirmLabel?: string;
  danger?: boolean;
  resolve: (ok: boolean) => void;
}

let show: ((r: Request) => void) | null = null;

/** Hộp thoại xác nhận trong app (thay cho window.confirm). */
export function askConfirm(opts: Omit<Request, "resolve">): Promise<boolean> {
  return new Promise((resolve) => {
    if (!show) return resolve(window.confirm(opts.title));
    show({ ...opts, resolve });
  });
}

export function ConfirmHost() {
  const [req, setReq] = useState<Request | null>(null);
  const okRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    show = setReq;
    return () => {
      show = null;
    };
  }, []);

  useEffect(() => {
    if (!req) return;
    okRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const close = (ok: boolean) => {
    req?.resolve(ok);
    setReq(null);
  };

  if (!req) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={() => close(false)}>
      <div
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
        className="w-[26rem] rounded-2xl border border-ink-700 bg-ink-900 p-6 shadow-2xl"
      >
        <h2 className="text-lg font-bold">{req.title}</h2>
        {req.message && <p className="mt-2 text-sm text-ink-300">{req.message}</p>}
        <div className="mt-6 flex justify-end gap-2">
          <Button variant="ghost" onClick={() => close(false)}>
            Huỷ
          </Button>
          <Button ref={okRef} variant={req.danger ? "record" : "primary"} onClick={() => close(true)}>
            {req.confirmLabel ?? "Đồng ý"}
          </Button>
        </div>
      </div>
    </div>
  );
}
