import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { send, Slider } from "./controls";
import { matchingPreset, MAX_PRESET_NAME, type AppState, type EqPreset } from "./types";

type Editor = { kind: "save" } | { kind: "rename"; id: string };

const SIDETONE = [
  { label: "Off", value: 0 },
  { label: "High", value: 1 },
  { label: "Med", value: 2 },
  { label: "Low", value: 3 },
];

/** Equalizer presets and bands, and sidetone (M4). Collapsible like the mockup. */
export function EqCard({
  state,
  presets,
  onPresets,
}: {
  state: AppState;
  presets: EqPreset[];
  /** The preset list after a save/rename/delete. */
  onPresets: (presets: EqPreset[]) => void;
}) {
  const [open, setOpen] = useState(() => readOpen());
  const values = state.eqBands.map((b) => b.value);
  const current = matchingPreset(presets, values);
  const eqBusy = state.busy?.startsWith("eq") ?? false;
  const hasEq = state.supportsEq && state.eqBands.length > 0;
  // The list always has the built-ins once loaded; until then don't call the EQ "Custom".
  const presetsLoaded = presets.length > 0;
  if (!hasEq && !state.supportsSidetone) return null;

  const toggle = () => {
    setOpen(!open);
    writeOpen(!open);
  };

  return (
    <section className="card">
      <button type="button" className="card-header collapse" aria-expanded={open} onClick={toggle}>
        <span className="card-title">Equalizer</span>
        <svg className={`chevron ${open ? "open" : ""}`} width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M3 1.5 6.5 5 3 8.5" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
        <span className="spacer" />
        {hasEq && presetsLoaded && (
          <span className={`preset-badge ${current ? "" : "unsaved"}`}>
            {current ? `${current.isBuiltIn ? "" : "★ "}${current.name}` : "Custom"}
          </span>
        )}
      </button>

      {open && (
        <div className="eq-body">
          {hasEq && (
            <>
              <div className="chips" role="group" aria-label="EQ presets">
                {presets.map((p) => (
                  <button
                    key={p.id}
                    type="button"
                    className={`chip ${p.id === current?.id ? "selected" : ""}`}
                    aria-pressed={p.id === current?.id}
                    disabled={eqBusy}
                    onClick={() => p.id !== current?.id && send({ type: "applyEq", args: p.values })}
                  >
                    {p.isBuiltIn ? p.name : `★ ${p.name}`}
                  </button>
                ))}
              </div>
              {state.eqBands.map((band) => (
                <Slider
                  key={band.bandId}
                  id={`eq-${band.name.toLowerCase()}`}
                  label={band.name}
                  labelWidth={48}
                  min={band.minimum}
                  max={band.maximum}
                  value={band.value}
                  disabled={eqBusy}
                  format={(v) => (v > 0 ? `+${v}` : String(v))}
                  onCommit={(value) => send({ type: "setEq", args: { bandId: band.bandId, value } })}
                />
              ))}
              {presetsLoaded && <PresetStatus current={current} values={values} onPresets={onPresets} />}
            </>
          )}
          {state.supportsSidetone && (
            <div className="editor-row sidetone">
              <span title="Hear your own voice during calls">Sidetone</span>
              <div className="segmented" role="radiogroup" aria-label="Sidetone">
                {SIDETONE.map((s) => (
                  <button
                    key={s.value}
                    type="button"
                    role="radio"
                    aria-checked={state.sidetone === s.value}
                    className={state.sidetone === s.value ? "selected" : ""}
                    disabled={state.busy === "sidetone"}
                    onClick={() => state.sidetone !== s.value && send({ type: "setSidetone", args: s.value })}
                  >
                    {s.label}
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </section>
  );
}

/** Under the sliders: Save for unsaved values, Rename/Delete for a custom preset, or the name editor. */
function PresetStatus({
  current,
  values,
  onPresets,
}: {
  current: EqPreset | undefined;
  values: number[];
  onPresets: (presets: EqPreset[]) => void;
}) {
  const [editor, setEditor] = useState<Editor | null>(null);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);

  const begin = (next: Editor, initial: string) => {
    setEditor(next);
    setName(initial);
    setError(null);
  };
  const submit = async () => {
    if (!editor) return;
    try {
      const updated =
        editor.kind === "save"
          ? await invoke<EqPreset[]>("save_preset", { name, values })
          : await invoke<EqPreset[]>("rename_preset", { id: editor.id, name });
      onPresets(updated);
      setEditor(null);
    } catch (e) {
      setError(String(e));
    }
  };

  if (editor) {
    return (
      <form
        className="preset-editor"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <input
          aria-label="Preset name"
          placeholder="Preset name"
          value={name}
          maxLength={MAX_PRESET_NAME}
          autoFocus
          onChange={(e) => {
            setName(e.target.value);
            setError(null);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.stopPropagation(); // Esc closes the editor, not the flyout
              setEditor(null);
            }
          }}
        />
        <button type="submit" className="link-button" disabled={!name.trim()}>
          {editor.kind === "save" ? "Save" : "Rename"}
        </button>
        <button type="button" className="link-button muted-link" onClick={() => setEditor(null)}>
          Cancel
        </button>
        {error && (
          <div className="field-error" role="alert">
            {error}
          </div>
        )}
      </form>
    );
  }

  if (!current) {
    return (
      <div className="preset-status">
        <button type="button" className="link-button" onClick={() => begin({ kind: "save" }, "")}>
          Save as preset…
        </button>
      </div>
    );
  }
  if (current.isBuiltIn) return null;
  return (
    <div className="preset-status">
      <span className="muted">Saved preset</span>
      <button type="button" className="link-button" onClick={() => begin({ kind: "rename", id: current.id }, current.name)}>
        Rename
      </button>
      <button
        type="button"
        className="link-button"
        onClick={async () => onPresets(await invoke<EqPreset[]>("delete_preset", { id: current.id }))}
      >
        Delete
      </button>
    </div>
  );
}

// The collapsed/expanded choice is a per-viewer convenience; storage may be unavailable.
function readOpen(): boolean {
  try {
    return localStorage.getItem("qcbuddy.eqOpen") !== "false";
  } catch {
    return true;
  }
}

function writeOpen(open: boolean) {
  try {
    localStorage.setItem("qcbuddy.eqOpen", String(open));
  } catch {
    // ignore
  }
}
