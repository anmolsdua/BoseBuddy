// The release page text for one version: its section of CHANGELOG.md
// (only what changed in that release) plus a short fixed footer.
//
//   node scripts/release-notes.mjs --version 0.1.3 --changelog ../CHANGELOG.md \
//     --repo hu13/BoseBuddy --out notes.md

import { readFileSync, writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

/** The body of `## <version>` in the changelog, without the heading. */
export function changelogSection(changelog, version) {
  const lines = changelog.replace(/\r\n/g, "\n").split("\n");
  const start = lines.findIndex((l) => l.trim() === `## ${version}`);
  if (start < 0) throw new Error(`CHANGELOG.md has no "## ${version}" section`);
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((l) => l.startsWith("## "));
  const body = (end < 0 ? rest : rest.slice(0, end)).join("\n").trim();
  if (!body) throw new Error(`CHANGELOG.md's "## ${version}" section is empty`);
  return body;
}

export function releaseNotes({ changelog, version, repo }) {
  return `${changelogSection(changelog, version)}

---

**First install?** Download \`QCBuddy_${version}_x64-setup.exe\` below; it installs for your user only, no admin needed. QC Buddy isn't code-signed yet, so Windows may show "Windows protected your PC": click **More info**, then **Run anyway**.

**Already on 0.1.2 or later?** QC Buddy offers this update itself (or right-click the tray icon → **Check for updates**).

Earlier changes: [CHANGELOG.md](https://github.com/${repo}/blob/main/windows/CHANGELOG.md)
`;
}

function args(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 2) out[argv[i].replace(/^--/, "")] = argv[i + 1];
  return out;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const a = args(process.argv.slice(2));
  const notes = releaseNotes({ changelog: readFileSync(a.changelog, "utf8"), version: a.version, repo: a.repo });
  writeFileSync(a.out, notes);
  console.log(notes);
}
