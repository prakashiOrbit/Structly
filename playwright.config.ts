import { defineConfig } from "@playwright/test";

// Runs against the plain web build (`vite preview`), not the Tauri shell — good enough
// for UI-flow smoke tests. True cross-platform desktop e2e needs tauri-driver + WebDriver,
// which is a separate setup once native builds are being shipped.
export default defineConfig({
  testDir: "./e2e",
  webServer: {
    command: "npm run preview -- --port 4173",
    port: 4173,
    reuseExistingServer: !process.env.CI,
  },
  use: {
    baseURL: "http://localhost:4173",
  },
});
