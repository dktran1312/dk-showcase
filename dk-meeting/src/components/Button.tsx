import { forwardRef, type ButtonHTMLAttributes } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger" | "record";

const styles: Record<Variant, string> = {
  primary: "bg-white text-ink-950 hover:bg-zinc-200",
  secondary: "bg-ink-800 text-white hover:bg-ink-700 border border-ink-700",
  ghost: "text-ink-300 hover:text-white hover:bg-ink-800",
  danger: "bg-transparent text-rec border border-rec/40 hover:bg-rec-soft",
  record: "bg-rec text-white hover:bg-red-600 shadow-[0_0_0_6px_var(--color-rec-soft)]",
};

type Props = ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant };

export const Button = forwardRef<HTMLButtonElement, Props>(function Button(
  { variant = "secondary", className = "", ...props },
  ref,
) {
  return (
    <button
      ref={ref}
      {...props}
      className={`inline-flex items-center justify-center gap-2 rounded-lg px-4 py-2 text-sm font-semibold transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${styles[variant]} ${className}`}
    />
  );
});
