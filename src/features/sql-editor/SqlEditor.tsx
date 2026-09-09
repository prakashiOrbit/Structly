import Editor from "@monaco-editor/react";
import { useState } from "react";
import { Button } from "@/components/ui/Button";
import { api } from "@/lib/tauri";
import type { QueryResult } from "@/lib/types";
import { ResultsGrid } from "@/features/table-view/ResultsGrid";

export function SqlEditor({ connectionId }: { connectionId: string }) {
  const [sql, setSql] = useState("SELECT 1;");
  const [result, setResult] = useState<QueryResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);

  const run = async () => {
    setRunning(true);
    setError(null);
    try {
      setResult(await api.executeQuery(connectionId, sql));
    } catch (err) {
      setError(String(err));
    } finally {
      setRunning(false);
    }
  };

  const cancel = async () => {
    try {
      await api.cancelQuery(connectionId);
    } catch (err) {
      setError(String(err));
    }
    // `running` clears itself once the cancelled `execute_query` invoke call above
    // rejects and `run()`'s own finally block runs — not here.
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="h-64 flex-none border-b border-[var(--color-divider)]">
        <Editor
          language="sql"
          value={sql}
          onChange={(value) => setSql(value ?? "")}
          theme="vs"
          options={{ fontSize: 13, minimap: { enabled: false }, fontFamily: "var(--font-mono)" }}
        />
      </div>
      <div className="flex flex-none items-center gap-2 border-b border-[var(--color-divider)] bg-[var(--color-surface)] px-4 py-1.5">
        {running ? (
          <Button variant="secondary" onClick={cancel} className="text-[var(--color-accent-2-700)]">
            Cancel
          </Button>
        ) : (
          <Button variant="primary" onClick={run}>
            Run
            <span className="ml-1 font-mono text-[10px] opacity-70">⌘↵</span>
          </Button>
        )}
      </div>
      {result && (
        <div className="flex-none border-b border-[var(--color-divider)] bg-[var(--color-surface)] px-4 py-1 font-mono text-[11px] text-[var(--color-neutral-600)]">
          {result.rows.length > 0
            ? `${result.rows.length} row${result.rows.length === 1 ? "" : "s"}`
            : `${result.rows_affected ?? 0} row${result.rows_affected === 1 ? "" : "s"} affected`}
          {" · "}
          {result.duration_ms}ms
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-auto">
        {error && <p className="p-4 text-[var(--color-accent-2-700)]">{error}</p>}
        {result && <ResultsGrid result={result} />}
      </div>
    </div>
  );
}
