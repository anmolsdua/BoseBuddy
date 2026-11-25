import { describe, expect, it } from "vitest";
import { buildFeed, WINDOWS_PLATFORM } from "./update-feed.mjs";

const base = {
  version: "0.1.2",
  signature: "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQ==\n",
  url: "https://github.com/hu13/BoseBuddy/releases/download/qcbuddy-v0.1.2/QCBuddy_0.1.2_x64-setup.exe",
  notes: "See the release notes.",
  pubDate: "2026-11-25T23:00:00.000Z",
};

describe("update feed", () => {
  it("has the shape the Tauri updater reads, for windows-x86_64", () => {
    expect(buildFeed(base)).toEqual({
      version: "0.1.2",
      notes: "See the release notes.",
      pub_date: "2026-11-25T23:00:00.000Z",
      platforms: {
        "windows-x86_64": {
          signature: "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQ==",
          url: base.url,
        },
      },
    });
    expect(WINDOWS_PLATFORM).toBe("windows-x86_64");
  });

  it("refuses a feed that would break updates", () => {
    expect(() => buildFeed({ ...base, version: "v0.1.2" })).toThrow(/x.y.z/);
    expect(() => buildFeed({ ...base, signature: "  " })).toThrow(/signature/);
    expect(() => buildFeed({ ...base, url: "http://example.com/x.exe" })).toThrow(/https/);
  });
});
