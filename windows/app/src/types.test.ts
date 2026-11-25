import { describe, expect, it } from "vitest";
import { ancLabel, audioDevices, displayName, holdsAudio, peerKind, type Mode, type Peer } from "./types";
import { state } from "./test/helpers";

const mode = (rawCnc: number, windBlock = false): Mode => ({ index: 0, name: "M", rawCnc, windBlock, editable: true });
const peer = (name: string, extra: Partial<Peer> = {}): Peer => ({
  mac: "AA",
  name,
  connected: false,
  isLocal: false,
  isBoseProduct: false,
  active: false,
  ...extra,
});

describe("ancLabel", () => {
  it("shows noise cancelling strength, 10 = full", () => {
    expect(ancLabel(mode(0))).toBe("ANC 10/10");
    expect(ancLabel(mode(10))).toBe("ANC 0/10");
    expect(ancLabel(mode(3))).toBe("ANC 7/10");
    expect(ancLabel(mode(42))).toBe("ANC 0/10");
  });
  it("says Auto under Wind Block", () => {
    expect(ancLabel(mode(0, true))).toBe("ANC Auto · Wind Block");
  });
});

describe("audio slots", () => {
  it("counts connected devices and this PC only when Windows has its audio", () => {
    const s = state("controlOnly");
    const pc = s.peers.find((p) => p.isLocal)!;
    expect(holdsAudio(pc, s)).toBe(false);
    expect(holdsAudio(pc, { ...s, pcAudio: true })).toBe(true);
  });
  it("lists who holds audio for the header", () => {
    expect(audioDevices(state("connected"))).toBe("PC + Sam's MacBook Pro");
    expect(audioDevices(state("controlOnly"))).toBe("Sam's MacBook Pro");
  });
});

describe("device names and icons", () => {
  it("calls the local entry This PC and falls back to the MAC", () => {
    expect(displayName(peer("DESKTOP-1", { isLocal: true }))).toBe("This PC");
    expect(displayName(peer("", { mac: "10:00:00:00:00:09" }))).toBe("10:00:00:00:00:09");
  });
  it("picks an icon like the core's Peer::kind", () => {
    expect(peerKind(peer("Sam’s iPhone"))).toBe("phone");
    expect(peerKind(peer("Pixel 9"))).toBe("phone");
    expect(peerKind(peer("iPad Air"))).toBe("tablet");
    expect(peerKind(peer("QC Earbuds", { isBoseProduct: true }))).toBe("headphones");
    expect(peerKind(peer("Work laptop"))).toBe("computer");
    expect(peerKind(peer("Phone", { isLocal: true }))).toBe("computer");
  });
});
