// Tauri's IPC doesn't exist in tests: invoke() is a mock that resolves to
// null (tests inspect its calls), and listen() keeps handlers so tests can
// emit events (see emit() in helpers.ts).
import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";

type Handler = (event: { payload: unknown }) => void;
const listeners = new Map<string, Handler>();
(globalThis as { __tauriListeners?: Map<string, Handler> }).__tauriListeners = listeners;

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve(null)) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: Handler) => {
    listeners.set(name, handler);
    return Promise.resolve(() => listeners.delete(name));
  }),
}));

// jsdom has no ResizeObserver; report one resize on observe().
class ResizeObserverStub {
  constructor(private callback: ResizeObserverCallback) {}
  observe() {
    this.callback([], this as unknown as ResizeObserver);
  }
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver = ResizeObserverStub as unknown as typeof ResizeObserver;

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
