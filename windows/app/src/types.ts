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
  busy: string | null;
  peerAction: string | null;
  actionNote: string | null;
  error: string | null;
}

export type Command =
  | { type: "flyoutOpened" }
  | { type: "flyoutClosed" }
  | { type: "connect"; args: string | null }
  | { type: "refresh" }
  | { type: "setMode"; args: number }
  | { type: "clearError" };

export function ancLabel(mode: Mode): string {
  if (mode.windBlock) return "ANC Auto · Wind Block";
  return `ANC ${10 - Math.min(10, Math.max(0, mode.rawCnc))}/10`;
}

/** "PC + iPhone": devices holding the headset's audio slots. */
export function audioDevices(state: AppState): string {
  return state.peers
    .filter((p) => p.connected || (p.isLocal && state.pcAudio))
    .map((p) => (p.isLocal ? "PC" : p.name || p.mac))
    .join(" + ");
}
