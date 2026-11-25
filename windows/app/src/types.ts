// Mirrors qcbuddy-core's `State` (serde camelCase) and `Command`.

export type LinkState =
  | { kind: "disconnected" }
  | { kind: "connecting" }
  | { kind: "connected" }
  | { kind: "failed"; message: string };

export interface Mode {
  index: number;
  name: string;
  /** 0 = full noise cancelling, 10 = fully aware. */
  rawCnc: number;
  windBlock: boolean;
  editable: boolean;
}

export interface Peer {
  mac: string;
  name: string;
  connected: boolean;
  isLocal: boolean;
  isBoseProduct: boolean;
  active: boolean;
}

export interface EqBand {
  bandId: number;
  name: string;
  minimum: number;
  maximum: number;
  value: number;
}

export interface Headphones {
  address: string;
  name: string;
  connected: boolean;
}

export interface AppState {
  link: LinkState;
  headphones: Headphones[];
  deviceName: string;
  productName: string;
  firmware: string;
  battery: number | null;
  charging: boolean;
  modes: Mode[];
  modeIndex: number | null;
  freeModeSlot: number | null;
  eqBands: EqBand[];
  multipoint: boolean;
  sidetone: number;
  peers: Peer[];
  activePeer: string | null;
  pairingMode: boolean;
  pcAudio: boolean;
  supportsModes: boolean;
  supportsEq: boolean;
  supportsSidetone: boolean;
  supportsMultipoint: boolean;
  supportsPeers: boolean;
  peersLoaded: boolean;
  pendingSwitch: PeerSwitch | null;
  busy: string | null;
  peerAction: string | null;
  actionNote: string | null;
  error: string | null;
}

/** What the app emits: the state with a revision that only goes up. */
export interface Snapshot {
  revision: number;
  state: AppState;
}

export interface PeerSwitch {
  target: Peer;
  candidates: Peer[];
}

export type Command =
  | { type: "connect"; args: string | null }
  | { type: "refresh" }
  | { type: "setMode"; args: number }
  | { type: "updateMode"; args: { index: number; rawCnc: number; windBlock: boolean } }
  | { type: "createCustomMode" }
  | { type: "setEq"; args: { bandId: number; value: number } }
  | { type: "applyEq"; args: number[] }
  | { type: "setSidetone"; args: number }
  | { type: "setMultipoint"; args: boolean }
  | { type: "connectPeer"; args: string }
  | { type: "disconnectPeer"; args: string }
  | { type: "confirmSwitch"; args: { dropMac: string } }
  | { type: "cancelSwitch" }
  | { type: "enterPairingMode" }
  | { type: "clearPairingMode" }
  | { type: "clearError" };

/** A named Bass/Mid/Treble setting (core's EqPreset). */
export interface EqPreset {
  id: string;
  name: string;
  values: number[];
  isBuiltIn: boolean;
  icon: string | null;
}

export const MAX_PRESET_NAME = 13;

/** The first preset whose values equal the headset's EQ (like EqPresetStore::preset_matching). */
export function matchingPreset(presets: EqPreset[], values: number[]): EqPreset | undefined {
  if (values.length === 0) return undefined;
  return presets.find((p) => p.values.length === values.length && p.values.every((v, i) => v === values[i]));
}

/** Noise cancelling strength shown to the user (10 = full); the headset stores 10 - level. */
export function ancLevel(mode: Mode): number {
  return 10 - Math.min(10, Math.max(0, mode.rawCnc));
}

export function ancLabel(mode: Mode): string {
  if (mode.windBlock) return "ANC Auto · Wind Block";
  return `ANC ${ancLevel(mode)}/10`;
}

/** Whether a device holds one of the headset's audio slots (same rule as the core). */
export function holdsAudio(peer: Peer, state: AppState): boolean {
  return peer.connected || (peer.isLocal && state.pcAudio);
}

/** "PC + iPhone": devices holding the headset's audio slots. */
export function audioDevices(state: AppState): string {
  return state.peers
    .filter((p) => holdsAudio(p, state))
    .map((p) => (p.isLocal ? "PC" : displayName(p)))
    .join(" + ");
}

export function displayName(peer: Peer): string {
  return peer.isLocal ? "This PC" : peer.name || peer.mac;
}

export type PeerKind = "computer" | "phone" | "tablet" | "headphones";

/** Icon choice, same rule as the core's Peer::kind. */
export function peerKind(peer: Peer): PeerKind {
  if (peer.isLocal) return "computer";
  const name = peer.name.toLowerCase();
  if (name.includes("ipad") || name.includes("pad")) return "tablet";
  if (name.includes("iphone") || name.includes("phone") || name.includes("pixel")) return "phone";
  if (peer.isBoseProduct) return "headphones";
  return "computer";
}
