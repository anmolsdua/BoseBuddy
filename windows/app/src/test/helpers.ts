import { act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import type { Mock } from "vitest";
import type { AppState, Command, EqPreset } from "../types";

import connected from "../__fixtures__/connected.json";
import controlOnly from "../__fixtures__/control-only.json";
import customMode from "../__fixtures__/custom-mode.json";
import eqUnsaved from "../__fixtures__/eq-unsaved.json";
import presetList from "../__fixtures__/presets.json";
import disconnected from "../__fixtures__/disconnected.json";
import error from "../__fixtures__/error.json";
import lowBattery from "../__fixtures__/low-battery.json";
import pairing from "../__fixtures__/pairing.json";
import pendingSwitch from "../__fixtures__/pending-switch.json";
import slotFree from "../__fixtures__/slot-free.json";

/** Real controller states from the core (see core/src/ui_fixtures.rs). */
export const fixtures = {
  connected,
  controlOnly,
  customMode,
  eqUnsaved,
  disconnected,
  error,
  lowBattery,
  pairing,
  pendingSwitch,
  slotFree,
} as unknown as Record<string, AppState>;

/** A fixture with some fields changed. */
export function state(name: keyof typeof fixtures, changes: Partial<AppState> = {}): AppState {
  return { ...structuredClone(fixtures[name]), ...changes };
}

export const invokeMock = invoke as unknown as Mock;

/** Commands the UI sent to the core, in order. */
export function sent(): Command[] {
  return invokeMock.mock.calls.filter(([name]) => name === "send").map(([, args]) => (args as { command: Command }).command);
}

/** Deliver a Tauri event to the UI, as the worker does. */
export function emit(name: string, payload: unknown) {
  const listeners = (globalThis as { __tauriListeners?: Map<string, (e: { payload: unknown }) => void> }).__tauriListeners!;
  act(() => listeners.get(name)?.({ payload }));
}

export const mac = (n: number) => `10:00:00:00:00:0${n}`;

/** The app's `presets` command result: built-ins plus a custom "Commute" [7, 4, 1]. */
export const presets = presetList as EqPreset[];
