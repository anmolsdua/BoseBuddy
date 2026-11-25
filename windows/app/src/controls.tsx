import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Command, PeerKind } from "./types";

export const SEND_ERROR_EVENT = "qcbuddy:send-error";

/**
 * Queue a command for the core. If it's refused (worker busy or stopped),
 * the flyout shows why instead of the click silently doing nothing.
 */
export const send = (command: Command) =>
  invoke("send", { command }).catch((e: unknown) => {
    window.dispatchEvent(new CustomEvent(SEND_ERROR_EVENT, { detail: String(e) }));
  });

export function Toggle({
  on,
  label,
  disabled,
  onChange,
}: {
  on: boolean;
  label: string;
  disabled?: boolean;
  onChange: (on: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      className={`toggle ${on ? "on" : ""}`}
      disabled={disabled}
      onClick={() => onChange(!on)}
    >
      <span />
    </button>
  );
}

/**
 * A range slider that follows the drag locally and sends one write when the
 * drag or key press ends (a write per step would flood the headset).
 */
export function Slider({
  id,
  label,
  min,
  max,
  value,
  disabled,
  format = (v) => String(v),
  labelWidth,
  onCommit,
}: {
  id: string;
  label: string;
  min: number;
  max: number;
  value: number;
  disabled?: boolean;
  format?: (value: number) => string;
  labelWidth?: number;
  onCommit: (value: number) => void;
}) {
  const [draft, setDraft] = useState(value);
  // Pointer-up then blur would send the same value twice before the headset answers.
  const sent = useRef<number | null>(null);
  useEffect(() => {
    setDraft(value);
    sent.current = null;
  }, [value]);
  const commit = () => {
    if (draft !== value && draft !== sent.current) {
      sent.current = draft;
      onCommit(draft);
    }
  };
  return (
    <div className="slider-row">
      <label htmlFor={id} style={labelWidth ? { width: labelWidth } : undefined}>
        {label}
      </label>
      <input
        id={id}
        type="range"
        min={min}
        max={max}
        step={1}
        value={draft}
        disabled={disabled}
        onChange={(e) => setDraft(Number(e.target.value))}
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
      />
      <span className="slider-value">{format(draft)}</span>
    </div>
  );
}

export function HeadphonesIcon({ size = 24 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M4 14v-2a8 8 0 0 1 16 0v2" />
      <rect x="3" y="14" width="4" height="6" rx="1.5" />
      <rect x="17" y="14" width="4" height="6" rx="1.5" />
    </svg>
  );
}

export function DeviceIcon({ kind }: { kind: PeerKind }) {
  const common = {
    width: 18,
    height: 18,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.6,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true,
  };
  switch (kind) {
    case "phone":
      return (
        <svg {...common}>
          <rect x="7" y="3" width="10" height="18" rx="2" />
          <path d="M11 18h2" />
        </svg>
      );
    case "tablet":
      return (
        <svg {...common}>
          <rect x="4" y="3" width="16" height="18" rx="2" />
          <path d="M11 18h2" />
        </svg>
      );
    case "headphones":
      return <HeadphonesIcon size={18} />;
    default:
      return (
        <svg {...common}>
          <rect x="3" y="5" width="18" height="12" rx="1.5" />
          <path d="M8 20h8" />
        </svg>
      );
  }
}
