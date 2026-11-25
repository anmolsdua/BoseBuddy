import { DeviceIcon, send, Toggle } from "./controls";
import { displayName, holdsAudio, peerKind, type AppState, type Peer } from "./types";

/** The headset's paired devices: connect, disconnect, switch (M3). */
export function DevicesCard({ state }: { state: AppState }) {
  const holding = state.peers.filter((p) => holdsAudio(p, state)).length;
  const slots = state.multipoint ? 2 : 1;
  const full = holding >= slots;
  const working = state.peerAction !== null || state.busy !== null;

  return (
    <section className="card">
      <div className="card-header">
        <div className="card-header-left">
          <span className="card-title">Devices</span>
          {state.supportsPeers && (
            <span className="chip-count" title="Devices using the headset's audio">
              {holding}/{slots} audio
            </span>
          )}
        </div>
        {state.supportsMultipoint && (
          <label className="inline-toggle">
            Multipoint
            <Toggle
              on={state.multipoint}
              label="Multipoint"
              disabled={state.busy === "multipoint"}
              onChange={(on) => send({ type: "setMultipoint", args: on })}
            />
          </label>
        )}
      </div>

      {!state.peersLoaded && <p className="card-text">Loading devices…</p>}
      {state.peersLoaded && !state.supportsPeers && (
        <p className="card-text">These headphones don't share their device list.</p>
      )}

      {state.pendingSwitch && <SwitchPrompt state={state} />}

      {state.peers.map((peer) => (
        <PeerRow key={peer.mac} peer={peer} state={state} full={full} working={working} />
      ))}

      {state.pairingMode && (
        <div className="pairing">
          <span>
            Pairing mode is on. On the new device, open Bluetooth settings and choose <b>{state.deviceName}</b>.
          </span>
          <button type="button" className="link-button" onClick={() => send({ type: "clearPairingMode" })}>
            Done
          </button>
        </div>
      )}
    </section>
  );
}

function PeerRow({ peer, state, full, working }: { peer: Peer; state: AppState; full: boolean; working: boolean }) {
  const holds = holdsAudio(peer, state);
  const busy = state.peerAction === peer.mac || (peer.isLocal && state.busy === "audio");

  let status: string;
  if (busy) status = state.actionNote ?? "Working…";
  else if (peer.isLocal) status = state.pcAudio ? (peer.active ? "This PC · Playing audio" : "This PC · Control + Audio") : "This PC · Control only";
  else if (peer.connected) status = peer.active ? "Connected · Playing audio" : "Connected";
  else status = "Not connected";

  let action: { label: string; primary: boolean; run: () => void } | null;
  if (peer.isLocal) {
    action = state.pcAudio
      ? { label: "Disconnect", primary: false, run: () => send({ type: "disconnectPeer", args: peer.mac }) }
      : { label: "Connect audio", primary: true, run: () => send({ type: "connectPeer", args: peer.mac }) };
  } else if (peer.connected) {
    action = { label: "Disconnect", primary: false, run: () => send({ type: "disconnectPeer", args: peer.mac }) };
  } else {
    action = { label: full ? "Switch" : "Connect", primary: true, run: () => send({ type: "connectPeer", args: peer.mac }) };
  }

  return (
    <div className={`peer ${busy ? "busy" : ""}`}>
      <div className="peer-icon">
        <DeviceIcon kind={peerKind(peer)} />
      </div>
      <div className="peer-text">
        <div className="peer-name">
          <span className={`dot ${holds ? "on" : "none"}`} />
          <span className="ellipsis" title={`${displayName(peer)} · ${peer.mac}`}>
            {displayName(peer)}
          </span>
        </div>
        <div className="peer-status">{status}</div>
      </div>
      {action && (
        <button
          type="button"
          className={`row-button ${action.primary ? "primary-text" : ""}`}
          disabled={working}
          title={action.label === "Switch" ? "Both audio slots are in use: drops a device that isn't this PC, then connects this one" : undefined}
          onClick={action.run}
        >
          {action.label}
        </button>
      )}
    </div>
  );
}

/** "Ask which" (setting): both slots are full, the user picks the device to drop. */
function SwitchPrompt({ state }: { state: AppState }) {
  const pending = state.pendingSwitch!;
  return (
    <div className="switch-prompt" role="dialog" aria-label="Choose a device to disconnect">
      <div>
        Both audio slots are in use. To connect <b>{displayName(pending.target)}</b>, disconnect:
      </div>
      <div className="switch-choices">
        {pending.candidates.map((c) => (
          <button key={c.mac} type="button" className="chip" onClick={() => send({ type: "confirmSwitch", args: { dropMac: c.mac } })}>
            {displayName(c)}
          </button>
        ))}
        <button type="button" className="link-button" onClick={() => send({ type: "cancelSwitch" })}>
          Cancel
        </button>
      </div>
    </div>
  );
}
