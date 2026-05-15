/**
 * Diagnostic error boundary.
 *
 * Catches React render errors that would otherwise produce a blank (white)
 * screen and forwards the full error + component stack to the Tauri OS log
 * stream via `@tauri-apps/plugin-log`. Tail with:
 *
 * ```
 * log stream --predicate 'subsystem == "com.swissarmyhammer.kanban" \
 *   AND composedMessage CONTAINS "[diag-error]"'
 * ```
 *
 * The boundary renders a minimal fallback panel so the window is not blank
 * after the crash — useful in development to show "something failed" rather
 * than an indistinguishable blank webview.
 */

import { Component, type ErrorInfo, type ReactNode } from "react";
import { error as logError } from "@tauri-apps/plugin-log";

interface DiagErrorBoundaryProps {
  children: ReactNode;
}

interface DiagErrorBoundaryState {
  error: Error | null;
  componentStack: string | null;
}

export class DiagErrorBoundary extends Component<
  DiagErrorBoundaryProps,
  DiagErrorBoundaryState
> {
  state: DiagErrorBoundaryState = { error: null, componentStack: null };

  static getDerivedStateFromError(error: Error): DiagErrorBoundaryState {
    return { error, componentStack: null };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    const stack = error.stack ?? "<no stack>";
    const componentStack = info.componentStack ?? "<no component stack>";
    this.setState({ componentStack });

    const msg =
      `[diag-error] ${error.name}: ${error.message}\n` +
      `stack:\n${stack}\n` +
      `componentStack:${componentStack}`;
    // Fire-and-forget — the underlying plugin forwards to the OS log.
    logError(msg).catch(() => {});
    // eslint-disable-next-line no-console
    console.error(msg);
  }

  render(): ReactNode {
    if (this.state.error) {
      return (
        <div
          style={{
            padding: 16,
            fontFamily: "ui-monospace, monospace",
            fontSize: 12,
            whiteSpace: "pre-wrap",
            background: "#1e0000",
            color: "#ffdddd",
            minHeight: "100vh",
            overflow: "auto",
          }}
        >
          <div style={{ fontWeight: "bold", marginBottom: 8 }}>
            React render error caught
          </div>
          <div>
            {this.state.error.name}: {this.state.error.message}
          </div>
          <pre style={{ marginTop: 8 }}>{this.state.error.stack}</pre>
          {this.state.componentStack ? (
            <pre style={{ marginTop: 8, opacity: 0.8 }}>
              Component stack:{this.state.componentStack}
            </pre>
          ) : null}
        </div>
      );
    }
    return this.props.children;
  }
}
