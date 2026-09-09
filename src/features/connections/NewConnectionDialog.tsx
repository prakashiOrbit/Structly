import { useState } from "react";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { useConnectionsStore, type NewConnectionInput } from "@/stores/useConnectionsStore";
import type { Engine } from "@/lib/types";

const ENGINES: { value: Engine; label: string; defaultPort: number | null }[] = [
  { value: "postgres", label: "PostgreSQL", defaultPort: 5432 },
  { value: "my_sql", label: "MySQL / MariaDB", defaultPort: 3306 },
  { value: "sqlite", label: "SQLite", defaultPort: null },
];

const emptyForm = {
  name: "",
  engine: "postgres" as Engine,
  host: "localhost",
  port: "5432",
  database: "",
  username: "",
  password: "",
  useSshTunnel: false,
  sshHost: "",
  sshPort: "22",
  sshUsername: "",
  sshPassword: "",
};

export function NewConnectionDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const create = useConnectionsStore((s) => s.create);
  const [form, setForm] = useState(emptyForm);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!open) return null;

  const isSqlite = form.engine === "sqlite";

  const set = <K extends keyof typeof form>(key: K, value: (typeof form)[K]) => setForm((f) => ({ ...f, [key]: value }));

  const selectEngine = (engine: Engine) => {
    const defaultPort = ENGINES.find((e) => e.value === engine)?.defaultPort;
    setForm((f) => ({ ...f, engine, port: defaultPort ? String(defaultPort) : "" }));
  };

  const submit = async () => {
    setSaving(true);
    setError(null);
    try {
      const input: NewConnectionInput = {
        name: form.name.trim() || form.database || "Untitled connection",
        engine: form.engine,
        host: isSqlite ? null : form.host || null,
        port: isSqlite ? null : form.port ? Number(form.port) : null,
        database: form.database.trim(),
        username: isSqlite ? null : form.username || null,
        password: isSqlite ? null : form.password || null,
        sshTunnel:
          !isSqlite && form.useSshTunnel
            ? {
                host: form.sshHost.trim(),
                port: Number(form.sshPort) || 22,
                username: form.sshUsername.trim(),
                password: form.sshPassword,
              }
            : null,
        readOnly: false,
      };
      await create(input);
      setForm(emptyForm);
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30"
      onClick={onClose}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="w-[420px] rounded-sm bg-[var(--color-surface)] p-5 shadow-lg"
      >
        <h3 className="mb-4 text-base font-semibold">New Connection</h3>

        <div className="flex gap-1.5 mb-4">
          {ENGINES.map((e) => (
            <button
              key={e.value}
              onClick={() => selectEngine(e.value)}
              className="flex-1 rounded-sm border px-2 py-1.5 text-[12px] font-medium"
              style={{
                borderColor: form.engine === e.value ? "var(--color-accent-600)" : "var(--color-divider)",
                background: form.engine === e.value ? "var(--color-accent-100)" : "transparent",
                color: form.engine === e.value ? "var(--color-accent-700)" : "var(--color-text)",
              }}
            >
              {e.label}
            </button>
          ))}
        </div>

        <div className="space-y-2.5">
          <Field label="Name">
            <Input
              className="w-full"
              placeholder="My database"
              value={form.name}
              onChange={(e) => set("name", e.target.value)}
            />
          </Field>

          {isSqlite ? (
            <Field label="File path">
              <Input
                className="w-full font-mono text-[12px]"
                placeholder="/path/to/database.sqlite"
                value={form.database}
                onChange={(e) => set("database", e.target.value)}
              />
            </Field>
          ) : (
            <>
              <div className="flex gap-2">
                <Field label="Host" className="flex-1">
                  <Input className="w-full" value={form.host} onChange={(e) => set("host", e.target.value)} />
                </Field>
                <Field label="Port" className="w-20">
                  <Input className="w-full" value={form.port} onChange={(e) => set("port", e.target.value)} />
                </Field>
              </div>
              <Field label="Database">
                <Input className="w-full" value={form.database} onChange={(e) => set("database", e.target.value)} />
              </Field>
              <div className="flex gap-2">
                <Field label="Username" className="flex-1">
                  <Input className="w-full" value={form.username} onChange={(e) => set("username", e.target.value)} />
                </Field>
                <Field label="Password" className="flex-1">
                  <Input
                    className="w-full"
                    type="password"
                    placeholder="required for most databases"
                    value={form.password}
                    onChange={(e) => set("password", e.target.value)}
                  />
                </Field>
              </div>
              {!form.password && (
                <p className="text-[11px] text-[var(--color-accent-2-700)]">
                  No password entered — leave this blank only if the database truly doesn't require one.
                </p>
              )}

              <label className="flex items-center gap-2 pt-1 text-[12px] font-medium text-[var(--color-neutral-700)]">
                <input
                  type="checkbox"
                  checked={form.useSshTunnel}
                  onChange={(e) => set("useSshTunnel", e.target.checked)}
                />
                Connect through an SSH tunnel
              </label>

              {form.useSshTunnel && (
                <div className="space-y-2.5 rounded-sm border border-[var(--color-divider)] p-2.5">
                  <div className="flex gap-2">
                    <Field label="SSH Host" className="flex-1">
                      <Input className="w-full" value={form.sshHost} onChange={(e) => set("sshHost", e.target.value)} />
                    </Field>
                    <Field label="SSH Port" className="w-20">
                      <Input className="w-full" value={form.sshPort} onChange={(e) => set("sshPort", e.target.value)} />
                    </Field>
                  </div>
                  <div className="flex gap-2">
                    <Field label="SSH Username" className="flex-1">
                      <Input className="w-full" value={form.sshUsername} onChange={(e) => set("sshUsername", e.target.value)} />
                    </Field>
                    <Field label="SSH Password" className="flex-1">
                      <Input
                        className="w-full"
                        type="password"
                        value={form.sshPassword}
                        onChange={(e) => set("sshPassword", e.target.value)}
                      />
                    </Field>
                  </div>
                  <p className="text-[11px] text-[var(--color-neutral-600)]">
                    Host/Port/Database above should be reachable <em>from the SSH server</em>, not necessarily from
                    this machine — e.g. an internal hostname on the bastion's own network.
                  </p>
                </div>
              )}
            </>
          )}
        </div>

        {error && <p className="mt-3 text-[12px] text-[var(--color-accent-2-700)]">{error}</p>}

        <div className="mt-5 flex justify-end gap-2">
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={submit} disabled={saving || !form.database.trim()}>
            {saving ? "Saving…" : "Save Connection"}
          </Button>
        </div>
      </div>
    </div>
  );
}

function Field({ label, className, children }: { label: string; className?: string; children: React.ReactNode }) {
  return (
    <label className={`block text-[11px] font-medium text-[var(--color-neutral-700)] ${className ?? ""}`}>
      {label}
      <span className="mt-0.5 block">{children}</span>
    </label>
  );
}
