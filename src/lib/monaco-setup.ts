// Points @monaco-editor/react at our own bundled copy of monaco-editor instead of its
// default behavior of fetching monaco from a CDN at runtime. This matters for a desktop
// app: it removes a network dependency for a core feature (the SQL editor won't work
// offline otherwise) and lets the app's CSP omit any CDN from script-src/connect-src.
//
// Only the base editor worker is registered — the "sql" language here is a Monarch
// (syntax-highlighting-only) grammar with no dedicated language worker. Add one here if a
// language that needs it (e.g. json, typescript) is ever wired into the editor.
import { loader } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";

self.MonacoEnvironment = {
  getWorker() {
    return new EditorWorker();
  },
};

loader.config({ monaco });
