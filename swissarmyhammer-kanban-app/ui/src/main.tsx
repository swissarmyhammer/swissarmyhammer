import React from "react";
import ReactDOM from "react-dom/client";
import { attachConsole } from "@tauri-apps/plugin-log";
import App from "./App";
import "./index.css";

// Redirect all console.log/warn/error through Tauri's log plugin
// so frontend logs appear in the same stderr/log file as Rust tracing.
attachConsole();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
