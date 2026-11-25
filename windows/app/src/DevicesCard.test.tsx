import { describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DevicesCard } from "./DevicesCard";
import { mac, sent, state } from "./test/helpers";
import type { AppState } from "./types";

function row(name: string | RegExp) {
  return screen.getByTitle(new RegExp(typeof name === "string" ? `^${name} ·` : name.source)).closest(".peer") as HTMLElement;
}

function button(name: string, label: string) {
  return within(row(name)).getByRole("button", { name: label });
}

const show = (s: AppState) => render(<DevicesCard state={s} />);

describe("DevicesCard", () => {
  it("lists devices with their status and slot count", () => {
    show(state("connected"));
    expect(screen.getByText("2/2 audio")).toBeTruthy();
    expect(within(row("This PC")).getByText("This PC · Control + Audio")).toBeTruthy();
    expect(within(row("Sam's MacBook Pro")).getByText("Connected · Playing audio")).toBeTruthy();
    expect(within(row("Sam’s iPhone")).getByText("Not connected")).toBeTruthy();
  });

  it("offers Switch when both slots are full, and sends connectPeer", async () => {
    show(state("connected"));
    expect(button("Sam's MacBook Air", "Switch")).toBeTruthy();
    await userEvent.click(button("Sam’s iPhone", "Switch"));
    expect(sent()).toEqual([{ type: "connectPeer", args: mac(3) }]);
  });

  it("offers Connect when a slot is free", () => {
    show(state("slotFree"));
    expect(screen.getByText("1/2 audio")).toBeTruthy();
    expect(button("Sam's MacBook Pro", "Connect")).toBeTruthy();
  });

  it("disconnects a connected device", async () => {
    show(state("connected"));
    await userEvent.click(button("Sam's MacBook Pro", "Disconnect"));
    expect(sent()).toEqual([{ type: "disconnectPeer", args: mac(1) }]);
  });

  it("offers Connect audio when this PC has only the control link", async () => {
    show(state("controlOnly"));
    expect(within(row("This PC")).getByText("This PC · Control only")).toBeTruthy();
    await userEvent.click(button("This PC", "Connect audio"));
    expect(sent()).toEqual([{ type: "connectPeer", args: mac(2) }]);
  });

  it("disconnects this PC's audio", async () => {
    show(state("connected"));
    await userEvent.click(button("This PC", "Disconnect"));
    expect(sent()).toEqual([{ type: "disconnectPeer", args: mac(2) }]);
  });

  it("shows progress and blocks other actions while one runs", () => {
    show(state("connected", { peerAction: mac(3), actionNote: "Switching from Sam's MacBook Pro…" }));
    expect(within(row("Sam’s iPhone")).getByText("Switching from Sam's MacBook Pro…")).toBeTruthy();
    for (const b of screen.getAllByRole("button").filter((b) => b.closest(".peer"))) {
      expect((b as HTMLButtonElement).disabled).toBe(true);
    }
  });

  it("asks which device to drop and sends the choice", async () => {
    show(state("pendingSwitch"));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("Sam’s iPhone")).toBeTruthy();
    await userEvent.click(within(dialog).getByRole("button", { name: "Sam's MacBook Pro" }));
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(sent()).toEqual([{ type: "confirmSwitch", args: { dropMac: mac(1) } }, { type: "cancelSwitch" }]);
  });

  it("toggles multipoint", async () => {
    show(state("pairing"));
    const toggle = screen.getByRole("switch", { name: "Multipoint" });
    expect(toggle.getAttribute("aria-checked")).toBe("false");
    await userEvent.click(toggle);
    expect(sent()).toEqual([{ type: "setMultipoint", args: true }]);
  });

  it("never shows more slots in use than slots (multipoint just turned off)", () => {
    show(state("pairing"));
    expect(screen.getByText("2/2 audio")).toBeTruthy();
  });

  it("explains pairing mode until Done", async () => {
    show(state("pairing"));
    expect(screen.getByText(/Pairing mode is on/).textContent).toContain("Bose QC45");
    await userEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(sent()).toEqual([{ type: "clearPairingMode" }]);
  });

  it("handles headphones that don't share their device list", () => {
    show(state("connected", { supportsPeers: false, peers: [] }));
    expect(screen.getByText("These headphones don't share their device list.")).toBeTruthy();
    expect(screen.queryByText(/audio$/)).toBeNull();
  });

  it("says when the list is still loading", () => {
    show(state("connected", { peersLoaded: false, peers: [] }));
    expect(screen.getByText("Loading devices…")).toBeTruthy();
  });
});
