import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ancLabel, audioDevices, type AppState, type Command, type Mode } from "./types";

const send = (command: Command) => invoke("send", { command });

export function App() {
  const [state, setState] = useState<AppState | null>(null);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invoke<AppState | null>("get_state").then((s) => s && setState(s));
    const unlisten = listen<AppState>("state", (e) => setState(e.payload));
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && invoke("hide_flyout");
    window.addEventListener("keydown", onKey);
    return () => {
      unlisten.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  // The window takes the height of its content, anchored above the tray.
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const fit = () => invoke("fit_flyout", { height: Math.ceil(el.getBoundingClientRect().height) });
    const observer = new ResizeObserver(fit);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return (
    <div className="flyout" ref={root}>
      {state ? <Flyout state={state} /> : <div className="loading">Starting…</div>}
    </div>
  );
}

function Flyout({ state }: { state: AppState }) {
  const connected = state.link.kind === "connected";
  return (
    <>
      <Header state={state} />
      {state.error && (
        <div className="error" role="alert">
          <span>{state.error}</span>
          <button type="button" aria-label="Dismiss" onClick={() => send({ type: "clearError" })}>
            ×
          </button>
        </div>
      )}
      <div className="divider" />
      <main className="cards">
        {connected ? <NoiseCard state={state} /> : <ConnectCard state={state} />}
      </main>
      <div className="divider" />
      <footer>
        <span className="muted">{connected && state.firmware ? `Firmware ${state.firmware}` : ""}</span>
        <button
          type="button"
          className="icon-button"
          aria-label="Refresh"
          title="Refresh"
          disabled={!connected}
          onClick={() => send({ type: "refresh" })}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
            <path d="M20 12a8 8 0 1 1-2.3-5.6" />
            <path d="M20 4v5h-5" />
          </svg>
        </button>
      </footer>
    </>
  );
}

function Header({ state }: { state: AppState }) {
  const { link } = state;
  let status: string;
  let dot: "on" | "busy" | "off";
  if (link.kind === "connected") {
    const devices = audioDevices(state);
    status = devices ? `Connected to: ${devices}` : "Connected";
    dot = "on";
  } else if (link.kind === "connecting") {
    status = "Connecting…";
    dot = "busy";
  } else {
    status = state.headphones.length ? "Not connected" : "No Bose headphones paired";
    dot = "off";
  }
  const battery = link.kind === "connected" ? state.battery : null;
  return (
    <header>
      <div className="app-icon">
        <HeadphonesIcon />
      </div>
      <div className="title">
        <div className="name">{state.deviceName}</div>
        <div className="status">
          <span className={`dot ${dot}`} />
          {status}
        </div>
      </div>
      {battery !== null && (
        <div className={`battery ${battery <= 20 ? "low" : ""}`} title={state.charging ? "Charging" : "Battery"}>
          {battery}%
        </div>
      )}
    </header>
  );
}

function NoiseCard({ state }: { state: AppState }) {
  const current = state.modes.find((m) => m.index === state.modeIndex);
  const switching = state.busy === "mode";
  const columns = Math.min(4, Math.max(1, state.modes.length));
  return (
    <section className="card">
      <div className="card-header">
        <span className="card-title">Noise control</span>
        {current && <span className="card-note">{ancLabel(current)}</span>}
      </div>
      <div className="modes" style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}>
        {state.modes.map((mode: Mode) => (
          <button
            key={mode.index}
            type="button"
            className={`mode ${mode.index === state.modeIndex ? "selected" : ""}`}
            aria-pressed={mode.index === state.modeIndex}
            disabled={switching}
            title={ancLabel(mode)}
            onClick={() => mode.index !== state.modeIndex && send({ type: "setMode", args: mode.index })}
          >
            {mode.name}
          </button>
        ))}
      </div>
    </section>
  );
}

function ConnectCard({ state }: { state: AppState }) {
  const connecting = state.link.kind === "connecting";
  const none = state.headphones.length === 0;
  return (
    <section className="card connect">
      <p>
        {none
          ? "Pair your Bose headphones in Settings → Bluetooth & devices, then come back here."
          : "Turn the headphones on and connect them to this PC, then click Connect."}
      </p>
      {!none && (
        <button type="button" className="primary" disabled={connecting} onClick={() => send({ type: "connect", args: null })}>
          {connecting ? "Connecting…" : "Connect"}
        </button>
      )}
    </section>
  );
}

function HeadphonesIcon() {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M4 14v-2a8 8 0 0 1 16 0v2" />
      <rect x="3" y="14" width="4" height="6" rx="1.5" />
      <rect x="17" y="14" width="4" height="6" rx="1.5" />
    </svg>
  );
}
