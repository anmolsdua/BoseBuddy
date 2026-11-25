import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { changelogSection, releaseNotes } from "./release-notes.mjs";

const changelog = `# QC Buddy changelog

Intro.

## 0.1.3

- Newest change.

## 0.1.2

- Older change.
`;

describe("release notes", () => {
  it("take only this version's changes", () => {
    expect(changelogSection(changelog, "0.1.3")).toBe("- Newest change.");
    expect(changelogSection(changelog, "0.1.2")).toBe("- Older change.");
    expect(changelogSection(changelog.replace(/\n/g, "\r\n"), "0.1.3")).toBe("- Newest change.");
  });

  it("refuse to release without notes", () => {
    expect(() => changelogSection(changelog, "0.1.4")).toThrow(/no "## 0.1.4" section/);
    expect(() => changelogSection("## 0.1.4\n\n## 0.1.3\n- x", "0.1.4")).toThrow(/empty/);
  });

  it("add the install footer and a link to the history", () => {
    const notes = releaseNotes({ changelog, version: "0.1.3", repo: "hu13/BoseBuddy" });
    expect(notes.startsWith("- Newest change.")).toBe(true);
    expect(notes).not.toContain("Older change");
    expect(notes).toContain("QCBuddy_0.1.3_x64-setup.exe");
    expect(notes).toContain("More info**, then **Run anyway");
    expect(notes).toContain("https://github.com/hu13/BoseBuddy/blob/main/windows/CHANGELOG.md");
  });

  it("has a section for the version being released", () => {
    // Tests run from windows/app.
    const real = readFileSync("../CHANGELOG.md", "utf8");
    const version = JSON.parse(readFileSync("package.json", "utf8")).version;
    expect(changelogSection(real, version).length).toBeGreaterThan(0);
  });
});
