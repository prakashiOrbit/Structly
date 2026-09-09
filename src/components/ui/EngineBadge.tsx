import { clsx } from "clsx";

const LABELS: Record<string, string> = {
  postgres: "PG",
  my_sql: "MYSQL",
  mysql: "MYSQL",
  sqlite: "SQLITE",
};

export function EngineBadge({ engine }: { engine: string }) {
  return (
    <span
      className={clsx(
        "font-mono text-[9px] tracking-widest text-[var(--color-neutral-600)]",
      )}
    >
      {LABELS[engine] ?? engine.toUpperCase()}
    </span>
  );
}
