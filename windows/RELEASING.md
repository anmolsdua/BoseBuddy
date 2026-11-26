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
   what changed since the last release. Open a pull request from a
   branch starting with `release/qcbuddy` (for example,
   `release/qcbuddy-v0.1.3`) in this repository into `main`.
   The release page shows that section plus a short install footer
   (`app/scripts/release-notes.mjs`); the release fails early if the section
   is missing.
2. Merge the PR. `windows-release.yml` checks the versions, runs the tests,
   builds and signs the installer, tags the exact merge commit as
   `qcbuddy-vX.Y.Z`, and creates a **draft** pre-release with:
   - `QCBuddy_X.Y.Z_x64-setup.exe` and its `.sig`
   - `latest-windows.json` (the update feed entry for this version)
   - `SHA256SUMS.txt`
   The tag version comes from `tauri.conf.json`, not the branch name.
   Closed-but-unmerged PRs and other source branches do not release.
   An existing tag is never moved: it must point to the same commit or
   the workflow fails. No extra token is needed; tagging and release
   creation happen in the same run.
3. Download the installer from the draft and check it (`HARDWARE-CHECKLIST.md`,
   install and update rows).
4. **Publish** the draft. `windows-publish-update.yml` copies its
   `latest-windows.json` to the permanent `qcbuddy-latest` release; installed
   copies are offered the update at their next check (at most 6 hours).
## Manual release fallback
Tag the intended commit with
`git tag -a qcbuddy-vX.Y.Z -m "QC Buddy X.Y.Z (Windows)"` and push that
tag. Tag pushes still run the same release workflow.

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

## Microsoft Store

The Store copy is the same build in an MSIX package (`app/msix/`,
`app/scripts/package-msix.ps1`), with the CLI as `qcb.exe` on PATH. The Store
signs it and updates it: the Store copy never reads the update feed, and
**Check for updates** / `qcb update --install` open its Store page instead
(`core/src/package.rs`).

**Once:**

- The listing text, privacy policy URL and other fields are in
  [`STORE-LISTING.md`](STORE-LISTING.md); the policy is
  [`PRIVACY.md`](PRIVACY.md). Update them when features or data handling
  change.
- In Partner Center, the product must be an **MSIX or PWA app** (an "EXE or
  MSI app" product only takes an installer URL; its type can't be changed).
- Copy QCBuddy → **Product identity** into repository variables:

  ```sh
  gh variable set MSIX_IDENTITY_NAME --repo hu13/BoseBuddy --body "<Package/Identity/Name>"
  gh variable set MSIX_PUBLISHER --repo hu13/BoseBuddy --body "<Package/Identity/Publisher, CN=...>"
  gh variable set MSIX_PUBLISHER_DISPLAY_NAME --repo hu13/BoseBuddy --body "<Package/Properties/PublisherDisplayName>"
  ```

  Without them the release still builds, with a placeholder MSIX for local
  testing that the Store rejects.

**Between releases:** Actions → **Windows MSIX** → Run workflow builds a
`.msix` from any branch, without tagging or releasing. Or build one on the
test PC: `npx tauri build` in `app`, `cargo build --release --bin qcbuddy` in
`core`, then `app\scripts\package-msix.ps1` with the three identity values
(output in `app\src-tauri\target\msix`).

**Each release**, after step 2 above:

1. Download `QCBuddy_X.Y.Z.0_x64.msix` from the release run's artifacts
   (Actions → Windows release → `qcbuddy-msix-X.Y.Z`).
2. Check it on the test PC (`HARDWARE-CHECKLIST.md`) by installing the
   unsigned package from the run: unzip the `.msix` (it's a zip) and, with
   Developer Mode on, `Add-AppxPackage -Register <folder>\AppxManifest.xml`.
   Uninstall the GitHub copy first, or both run side by side with separate
   settings. Optionally run the Windows App Certification Kit on the `.msix`.
3. Partner Center → QCBuddy → new submission → **Packages**: upload the
   `.msix`, then submit. Certification usually takes 1–3 business days.

The Store wants four-part versions ending in `.0` (`X.Y.Z.0`), each higher
than the last; a rejected submission needs a version bump.

**Package locations:** the Store copy's log and presets are under
`%LOCALAPPDATA%\Packages\<package family name>\LocalCache\Local\QC Buddy`
(Windows redirects the app's `%LOCALAPPDATA%\QC Buddy`). `qcb log` from the
Store copy shows the same folder, since it runs inside the package.
