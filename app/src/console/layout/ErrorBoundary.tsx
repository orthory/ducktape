// A top-level error boundary + (optional) global error sink. The app had NO
// boundary at all, so any render-time throw — a malformed projection, a null
// deref, a version-skewed shape — unmounted the whole tree to a blank white
// window with the message only in devtools (closed by default in the dev app).
//
// Two failure classes, two treatments. A render THROW breaks the tree, so it
// shows a full "Something crashed" fallback (message + stack + Reload), keeping
// whatever chrome sits above this boundary. A GLOBAL unhandled error/rejection
// (an escaped async failure) does not break the current render, so it shows a
// dismissible banner instead of nuking the UI. Only the boundary marked `global`
// attaches the window listeners, so nested boundaries don't double-surface.

import { Component, type CSSProperties, type ErrorInfo, type ReactNode } from "react";

import { color, font, radius } from "../theme/tokens";

interface Props {
  children: ReactNode;
  /** Attach window error/unhandledrejection listeners (set on the outermost). */
  global?: boolean;
}
interface State {
  renderError: Error | null;
  globalError: string | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { renderError: null, globalError: null };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { renderError: error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error("[ducktape] render error:", error, info.componentStack);
  }

  componentDidMount(): void {
    if (!this.props.global) return;
    window.addEventListener("error", this.onError);
    window.addEventListener("unhandledrejection", this.onRejection);
  }

  componentWillUnmount(): void {
    if (!this.props.global) return;
    window.removeEventListener("error", this.onError);
    window.removeEventListener("unhandledrejection", this.onRejection);
  }

  private onError = (ev: ErrorEvent): void => {
    // WebKit fires a benign "ResizeObserver loop…" as a global error — never a
    // real app failure, so don't surface it.
    if (ev.message && ev.message.includes("ResizeObserver")) return;
    const msg = ev.error instanceof Error ? ev.error.message : ev.message;
    if (msg) this.setState({ globalError: String(msg) });
  };

  private onRejection = (ev: PromiseRejectionEvent): void => {
    const r: unknown = ev.reason;
    const msg =
      r instanceof Error ? r.message : typeof r === "string" ? r : safeStringify(r);
    this.setState({ globalError: String(msg) });
  };

  render(): ReactNode {
    const { renderError, globalError } = this.state;
    if (renderError) {
      return (
        <div style={fill}>
          <div style={card}>
            <div style={{ font: `600 13px ${font.sans}`, color: color.danger, marginBottom: 6 }}>
              Something crashed
            </div>
            <div style={reasonStyle}>{renderError.message || String(renderError)}</div>
            {renderError.stack && <pre style={stackStyle}>{renderError.stack}</pre>}
            <button onClick={() => window.location.reload()} style={reloadBtn}>
              Reload
            </button>
          </div>
        </div>
      );
    }
    return (
      <>
        {this.props.children}
        {globalError && (
          <div style={banner}>
            <span
              style={{
                flex: 1,
                minWidth: 0,
                userSelect: "text",
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
              title={globalError}
            >
              {globalError}
            </span>
            <button
              onClick={() => this.setState({ globalError: null })}
              style={bannerX}
              aria-label="Dismiss"
            >
              ×
            </button>
          </div>
        )}
      </>
    );
  }
}

function safeStringify(value: unknown): string {
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

const fill: CSSProperties = {
  position: "absolute",
  inset: 0,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  padding: 24,
  background: color.paper,
  overflow: "auto",
  zIndex: 50,
};
const card: CSSProperties = {
  width: 560,
  maxWidth: "100%",
  border: `1px solid ${color.dangerBorder}`,
  background: color.dangerSoft,
  borderRadius: radius.lg,
  padding: 20,
};
const reasonStyle: CSSProperties = {
  font: `500 11.5px ${font.mono}`,
  color: color.ink,
  whiteSpace: "pre-wrap",
  wordBreak: "break-word",
  userSelect: "text",
  marginBottom: 12,
};
const stackStyle: CSSProperties = {
  margin: "0 0 14px",
  maxHeight: 240,
  overflow: "auto",
  background: color.paper,
  border: `1px solid ${color.border}`,
  borderRadius: radius.md,
  padding: 10,
  font: `500 10px ${font.mono}`,
  color: color.muted3,
  whiteSpace: "pre-wrap",
  wordBreak: "break-word",
  userSelect: "text",
};
const reloadBtn: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  font: `600 11px ${font.sans}`,
  color: "#fff",
  background: color.danger,
  borderRadius: radius.md,
  padding: "7px 14px",
};
const banner: CSSProperties = {
  position: "fixed",
  bottom: 12,
  left: "50%",
  transform: "translateX(-50%)",
  maxWidth: "min(680px, 92vw)",
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "8px 12px",
  background: color.dark,
  color: color.onDark,
  borderRadius: radius.md,
  font: `500 11px ${font.mono}`,
  zIndex: 60,
};
const bannerX: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  color: color.onDark,
  font: `600 14px ${font.sans}`,
  lineHeight: 1,
  padding: "0 2px",
};
