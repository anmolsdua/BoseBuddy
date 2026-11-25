import { send, Slider, Toggle } from "./controls";
import { ancLabel, ancLevel, type AppState, type Mode } from "./types";

/** Mode switching (M2), editing and "+ Custom" (M4). */
export function NoiseCard({ state }: { state: AppState }) {
  const current = state.modes.find((m) => m.index === state.modeIndex);
  const busy = state.busy !== null;
  const canCreate = state.freeModeSlot !== null;
  const columns = Math.min(4, Math.max(1, state.modes.length + (canCreate ? 1 : 0)));
  return (
    <section className="card">
      <div className="card-header">
        <span className="card-title">Noise control</span>
        {current && <span className="card-note">{ancLabel(current)}</span>}
      </div>
      <div className="modes" style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}>
        {state.modes.map((mode) => (
          <button
            key={mode.index}
            type="button"
            className={`mode ${mode.index === state.modeIndex ? "selected" : ""}`}
            aria-pressed={mode.index === state.modeIndex}
            disabled={busy}
            title={ancLabel(mode)}
            onClick={() => mode.index !== state.modeIndex && send({ type: "setMode", args: mode.index })}
          >
            {mode.name}
          </button>
        ))}
        {canCreate && (
          <button
            type="button"
            className="mode create"
            disabled={busy}
            title="Create a Custom mode from the current noise cancelling level"
            onClick={() => send({ type: "createCustomMode" })}
          >
            + Custom
          </button>
        )}
      </div>
      {current?.editable && <ModeEditor key={current.index} mode={current} disabled={busy} />}
      {(state.busy === "mode" || state.busy === "mode-edit") && (
        <div className="busy-note">{state.busy === "mode" ? "Switching…" : "Saving to headphones…"}</div>
      )}
    </section>
  );
}

/** Wind Block and noise cancelling for an editable (user) mode. Saved on the headset. */
function ModeEditor({ mode, disabled }: { mode: Mode; disabled: boolean }) {
  const update = (level: number, windBlock: boolean) =>
    send({ type: "updateMode", args: { index: mode.index, rawCnc: 10 - level, windBlock } });
  // Under Wind Block the headset stores CNC 0; turning it off starts from full ANC.
  const level = ancLevel(mode);
  return (
    <div className="mode-editor">
      <div className="editor-row">
        <span>Wind Block</span>
        <Toggle on={mode.windBlock} label="Wind Block" disabled={disabled} onChange={(on) => update(level, on)} />
      </div>
      <div className={mode.windBlock ? "dimmed" : undefined}>
        <Slider
          id="anc"
          label="Noise cancelling"
          min={0}
          max={10}
          value={level}
          disabled={disabled || mode.windBlock}
          format={(v) => (mode.windBlock ? "Auto" : String(v))}
          onCommit={(v) => update(v, false)}
        />
      </div>
    </div>
  );
}
