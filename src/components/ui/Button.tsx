import type { ButtonHTMLAttributes } from "react";
import { clsx } from "clsx";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost";
}

const variants: Record<NonNullable<ButtonProps["variant"]>, string> = {
  primary: "bg-[var(--color-accent-600)] text-white hover:bg-[var(--color-accent-700)]",
  secondary:
    "bg-[var(--color-surface)] text-[var(--color-text)] border border-[var(--color-divider)] hover:bg-[var(--color-neutral-200)]",
  ghost: "bg-transparent text-[var(--color-accent-700)] hover:bg-[var(--color-neutral-200)]",
};

export function Button({ variant = "secondary", className, ...props }: ButtonProps) {
  return (
    <button
      className={clsx(
        "inline-flex items-center gap-1.5 rounded-sm px-2.5 py-1 text-[12.5px] font-medium transition-colors",
        variants[variant],
        className,
      )}
      {...props}
    />
  );
}
