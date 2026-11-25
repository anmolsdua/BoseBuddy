import { invoke } from "@tauri-apps/api/core";
import type { UpdateStatus } from "./types";

/** "QC Buddy 0.1.3 is available · Install and restart", then progress. */
export function UpdateBanner({ status }: { status: UpdateStatus }) {
  const install = () => {
    // Errors come back as a "failed" status event.
    invoke("install_update").catch(() => {});
  };
  const dismiss = () => {
    invoke("dismiss_update_status").catch(() => {});
  };
  switch (status.kind) {
    case "none":
      return null;
    case "checking":
      return (
        <div className="update" role="status">
          <span>Checking for updates…</span>
        </div>
      );
    case "upToDate":
      return (
        <div className="update" role="status">
          <span>QC Buddy {status.version} is up to date.</span>
          <button type="button" className="link-button" onClick={dismiss}>
            OK
          </button>
        </div>
      );
    case "available":
      return (
        <div className="update" role="status">
          <span>QC Buddy {status.version} is available.</span>
          <button type="button" className="link-button" onClick={install}>
            Install and restart
          </button>
        </div>
      );
    case "downloading":
      return (
        <div className="update" role="status">
          <span>Downloading the update…{status.percent !== null ? ` ${status.percent}%` : ""}</span>
        </div>
      );
    case "installing":
      return (
        <div className="update" role="status">
          <span>Installing the update; QC Buddy will restart.</span>
        </div>
      );
    case "failed":
      return (
        <div className="update failed" role="alert">
          <span>{status.message}</span>
          <button
            type="button"
            className="link-button"
            onClick={() =>
              status.retry === "check" ? invoke("check_for_updates").catch(() => {}) : install()
            }
          >
            Try again
          </button>
          <button type="button" className="link-button" aria-label="Dismiss" onClick={dismiss}>
            ×
          </button>
        </div>
      );
  }
}
