import { describe, expect, it } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App, Flyout } from "./App";
import { listen } from "@tauri-apps/api/event";
import type { Mock } from "vitest";
import { emit, fixtures, invokeMock, sent, snapshot, state } from "./test/helpers";

describe("Header", () => {
  it("shows the headset, battery and who has audio", () => {
    render(<Flyout state={state("connected")} />);
    expect(screen.getByText("Bose QC45").getAttribute("title")).toBe("Firmware 4.0.4-4360+de6a887");
    expect(screen.getByText("80%").className).not.toContain("low");
    expect(screen.getByText("Connected to: PC + Sam's MacBook Pro")).toBeTruthy();
  });

  it("marks a low battery", () => {
    render(<Flyout state={state("lowBattery")} />);
    expect(screen.getByText("15%").className).toContain("low");
  });

  it("hides the battery and explains when not connected", () => {
    render(<Flyout state={state("disconnected")} />);
    expect(screen.getByText("Not connected")).toBeTruthy();
    expect(screen.queryByText(/%$/)).toBeNull();
  });

  it("says Connecting… while connecting", () => {
    render(<Flyout state={state("disconnected", { link: { kind: "connecting" } })} />);
    expect(screen.getAllByText("Connecting…").length).toBe(2); // header and button
  });
});

describe("Noise control", () => {
  it("highlights the current mode and its ANC level", () => {
    render(<Flyout state={state("lowBattery")} />);
    const walk = screen.getByRole("button", { name: "Walk" });
    expect(walk.getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByText("ANC Auto · Wind Block")).toBeTruthy();
  });

  it("switches modes, but not to the current one", async () => {
    render(<Flyout state={state("connected")} />);
    await userEvent.click(screen.getByRole("button", { name: "Quiet" }));
    await userEvent.click(screen.getByRole("button", { name: "Aware" }));
    expect(sent()).toEqual([{ type: "setMode", args: 1 }]);
  });

  it("disables mode buttons while switching", () => {
    render(<Flyout state={state("connected", { busy: "mode" })} />);
    expect((screen.getByRole("button", { name: "Aware" }) as HTMLButtonElement).disabled).toBe(true);
  });
});

describe("Flyout", () => {
  it("shows an error and dismisses it", async () => {
    render(<Flyout state={state("error")} />);
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("Sam's MacBook Air didn't answer.");
    await userEvent.click(within(alert).getByRole("button", { name: "Dismiss" }));
    expect(sent()).toEqual([{ type: "clearError" }]);
  });

  it("offers Connect when disconnected, and nothing that needs a connection", async () => {
    render(<Flyout state={state("disconnected")} />);
    expect(screen.queryByText("Devices")).toBeNull();
    expect(screen.queryByText("Noise control")).toBeNull();
    expect(screen.queryByText("+ Pair new device")).toBeNull();
    expect((screen.getByRole("button", { name: "Refresh" }) as HTMLButtonElement).disabled).toBe(true);
    await userEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect(sent()).toEqual([{ type: "connect", args: null }]);
  });

  it("tells you to pair first when no headset is paired", () => {
    render(<Flyout state={state("disconnected", { headphones: [] })} />);
    expect(screen.getByText("No Bose headphones paired")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Connect" })).toBeNull();
  });

  it("starts pairing from the footer, and hides the link while pairing", async () => {
    const { unmount } = render(<Flyout state={state("connected")} />);
    await userEvent.click(screen.getByRole("button", { name: "+ Pair new device" }));
    await userEvent.click(screen.getByRole("button", { name: "Refresh" }));
    expect(sent()).toEqual([{ type: "enterPairingMode" }, { type: "refresh" }]);
    unmount();
    render(<Flyout state={state("pairing")} />);
    expect(screen.queryByText("+ Pair new device")).toBeNull();
  });
});

describe("App", () => {
  it("loads the current state, then follows state events", async () => {
    invokeMock.mockImplementation((name: string) => Promise.resolve(name === "get_state" ? snapshot(fixtures.connected, 1) : null));
    render(<App />);
    expect(await screen.findByText("80%")).toBeTruthy();
    emit("state", snapshot(fixtures.lowBattery, 2));
    expect(screen.getByText("15%")).toBeTruthy();
    emit("state", snapshot(fixtures.disconnected, 3));
    expect(screen.getByText("Not connected")).toBeTruthy();
  });

  it("subscribes before asking for the snapshot, so no update is missed", async () => {
    render(<App />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_state"));
    const listenOrder = (listen as unknown as Mock).mock.invocationCallOrder[0];
    const getStateOrder = invokeMock.mock.invocationCallOrder[invokeMock.mock.calls.findIndex(([n]) => n === "get_state")];
    expect(listenOrder).toBeLessThan(getStateOrder);
  });

  it("never lets an older snapshot replace a newer event", async () => {
    let resolveSnapshot: (s: unknown) => void = () => {};
    invokeMock.mockImplementation((name: string) =>
      name === "get_state" ? new Promise((r) => (resolveSnapshot = r)) : Promise.resolve(null),
    );
    render(<App />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_state"));
    emit("state", snapshot(fixtures.lowBattery, 5)); // a change arrives first
    expect(screen.getByText("15%")).toBeTruthy();
    await act(async () => resolveSnapshot(snapshot(fixtures.connected, 4))); // then the older snapshot
    expect(screen.getByText("15%")).toBeTruthy();
    expect(screen.queryByText("80%")).toBeNull();
    emit("state", snapshot(fixtures.connected, 5)); // same revision again: ignored too
    expect(screen.getByText("15%")).toBeTruthy();
  });

  it("shows why a command was refused, and dismisses it locally", async () => {
    invokeMock.mockImplementation((name: string) =>
      name === "get_state"
        ? Promise.resolve(snapshot(fixtures.connected))
        : name === "send"
          ? Promise.reject("QC Buddy is still working on earlier changes. Try again in a moment.")
          : Promise.resolve(null),
    );
    render(<App />);
    await screen.findByText("80%");
    await userEvent.click(screen.getByRole("button", { name: "Aware" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("still working on earlier changes");
    invokeMock.mockClear();
    await userEvent.click(within(alert).getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(sent(), "the core's error wasn't touched").toEqual([]);
  });

  it("re-reads EQ presets whenever the flyout gets focus (the CLI may have saved one)", async () => {
    invokeMock.mockImplementation((name: string) => Promise.resolve(name === "get_state" ? snapshot(fixtures.connected) : name === "presets" ? [] : null));
    render(<App />);
    await screen.findByText("80%");
    const presetCalls = () => invokeMock.mock.calls.filter(([name]) => name === "presets").length;
    expect(presetCalls()).toBe(1);
    window.dispatchEvent(new Event("focus"));
    expect(presetCalls()).toBe(2);
  });

  it("shows Starting… until the core reports", () => {
    render(<App />);
    expect(screen.getByText("Starting…")).toBeTruthy();
  });

  it("sizes the window to its content and hides on Esc", async () => {
    render(<App />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("fit_flyout", { height: expect.any(Number) }));
    await userEvent.keyboard("{Escape}");
    expect(invokeMock).toHaveBeenCalledWith("hide_flyout");
  });
});
