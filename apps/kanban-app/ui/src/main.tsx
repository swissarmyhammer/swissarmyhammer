import React from "react";
import ReactDOM from "react-dom/client";
import { attachConsole } from "@tauri-apps/plugin-log";
import * as tauriLog from "@tauri-apps/plugin-log";
import App from "./App";
import { initUiResponders } from "./lib/ui-request-responder";
import "./index.css";

// attachConsole forwards Rust-side logs back to the JS console (display only).
attachConsole();

// Wrap native console methods so JS console.log/warn/error/debug also send
// to the Rust log plugin, which routes them to oslog via tracing.
const origLog = console.log;
const origWarn = console.warn;
const origError = console.error;
const origDebug = console.debug;

console.log = (...args: unknown[]) => {
  origLog(...args);
  tauriLog.info(args.map(String).join(" ")).catch(() => {});
};
console.warn = (...args: unknown[]) => {
  origWarn(...args);
  tauriLog.warn(args.map(String).join(" ")).catch(() => {});
};
console.error = (...args: unknown[]) => {
  origError(...args);
  tauriLog.error(args.map(String).join(" ")).catch(() => {});
};
console.debug = (...args: unknown[]) => {
  origDebug(...args);
  tauriLog.debug(args.map(String).join(" ")).catch(() => {});
};

// Establish the host→UI request/reply listener: the host emits `ui/request`
// to ask the webview a question (e.g. a live focus-geometry read) and awaits
// a correlated reply. Errors here must not block app render.
initUiResponders().catch((err) => {
  console.error("[main] failed to init ui responders:", err);
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
