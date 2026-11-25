# Releasing QC Buddy (Windows)

## Once: the update signing key

Installed copies only accept updates signed with the key whose public half
is in `app/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).

- The key pair was made with `npx tauri signer generate` and is kept outside
  the repo: `%USERPROFILE%\.qcbuddy-updater\` (`qcbuddy-updater.key`,
  `.key.pub`, `password.txt`). **Keep an offline copy (e.g. a password
  manager).** If the private key is lost, installed copies reject every
  future update and users have to reinstall by hand once.
- The release workflow reads it from two repository secrets:

  ```powershell
  Get-Content -Raw "$env:USERPROFILE\.qcbuddy-updater\qcbuddy-updater.key" | gh secret set TAURI_SIGNING_PRIVATE_KEY --repo hu13/BoseBuddy
  Get-Content -Raw "$env:USERPROFILE\.qcbuddy-updater\password.txt" | gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo hu13/BoseBuddy
  ```

## Each release

1. Bump the version in `app/src-tauri/tauri.conf.json`, `app/package.json`
   (`npm version x.y.z --no-git-tag-version`), `app/src-tauri/Cargo.toml` and
   `core/Cargo.toml`; add a `## X.Y.Z` section to `CHANGELOG.md` with only
   what changed since the last release. Merge to `main` via a pull request.
   The release page shows that section plus a short install footer
   (`app/scripts/release-notes.mjs`); the release fails early if the section
   is missing.
2. Tag the merge commit on `main`: `git tag -a qcbuddy-vX.Y.Z -m "QC Buddy X.Y.Z (Windows)"`
   and push the tag. `windows-release.yml` checks the versions, runs the
   tests, builds and signs the installer, and creates a **draft**
   pre-release with:
   - `QCBuddy_X.Y.Z_x64-setup.exe` and its `.sig`
   - `latest-windows.json` (the update feed entry for this version)
   - `SHA256SUMS.txt`
3. Download the installer from the draft and check it (`HARDWARE-CHECKLIST.md`,
   install and update rows).
4. **Publish** the draft. `windows-publish-update.yml` copies its
   `latest-windows.json` to the permanent `qcbuddy-latest` release; installed
   copies are offered the update at their next check (at most 6 hours).

## The update feed

- Installed copies read
  `https://github.com/hu13/BoseBuddy/releases/download/qcbuddy-latest/latest-{{target}}.json`;
  on Windows that's `latest-windows.json`. A future macOS build would read
  `latest-darwin.json`, so each platform can release on its own schedule.
- `qcbuddy-latest` is a pre-release that is never marked "Latest" (the
  macOS app's releases are). Don't delete it.
- **Roll back:** run "Windows update feed" by hand (Actions → Run workflow)
  with an older tag. Copies already on the newer version stay there (the
  updater only offers newer versions), but nobody else is offered it.
- Drafts are never offered: the feed changes only when a release is
  published.
- **Check by hand:** `QCBuddy.exe --check-for-updates` (the running copy
  checks and shows the result in the flyout), or right-click the tray icon →
  **Check for updates**.
