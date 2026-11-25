import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { HeadphonesIcon, send } from "./controls";
import { DevicesCard } from "./DevicesCard";
import { EqCard } from "./EqCard";
import { NoiseCard } from "./NoiseCard";
import { audioDevices, type AppState, type EqPreset } from "./types";

export function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [presets, setPresets] = useState<EqPreset[]>([]);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invoke<AppState | null>("get_state").then((s) => s && setState(s));
    // Presets live in a file the CLI can change too: re-read whenever the flyout shows.
    const loadPresets = () => invoke<EqPreset[] | null>("presets").then((p) => p && setPresets(p));
    loadPresets();
    window.addEventListener("focus", loadPresets);
    const unlisten = listen<AppState>("state", (e) => setState(e.payload));
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && invoke("hide_flyout");
    window.addEventListener("keydown", onKey);
    return () => {
      unlisten.then((f) => f());
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("focus", loadPresets);
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
      {state ? <Flyout state={state} presets={presets} onPresets={setPresets} /> : <div className="loading">Starting…</div>}
    </div>
  );
}

export function Flyout({
  state,
  presets = [],
  onPresets = () => {},
}: {
  state: AppState;
  presets?: EqPreset[];
  onPresets?: (presets: EqPreset[]) => void;
}) {
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
        {connected ? (
          <>
            <DevicesCard state={state} />
            <NoiseCard state={state} />
            <EqCard state={state} presets={presets} onPresets={onPresets} />
          </>
        ) : (
          <ConnectCard state={state} />
        )}
      </main>
      <div className="divider" />
      <footer>
        {connected && state.supportsPeers && !state.pairingMode ? (
          <button
            type="button"
            className="link-button"
            disabled={state.busy === "pairing"}
            title="Puts the headphones in pairing mode so a new device can find them"
            onClick={() => send({ type: "enterPairingMode" })}
          >
            + Pair new device
          </button>
        ) : (
          <span />
        )}
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

export function Header({ state }: { state: AppState }) {
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
        <div className="name" title={state.firmware ? `Firmware ${state.firmware}` : undefined}>
          {state.deviceName}
        </div>
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

export function ConnectCard({ state }: { state: AppState }) {
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
