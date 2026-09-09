import type { InputHTMLAttributes } from "react";
import { clsx } from "clsx";

export function Input({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={clsx(
        "rounded-sm border border-[var(--color-divider)] bg-[var(--color-bg)] px-2 py-1 text-[12.5px]",
        "outline-none focus:border-[var(--color-accent-600)]",
        className,
      )}
      {...props}
    />
  );
}
