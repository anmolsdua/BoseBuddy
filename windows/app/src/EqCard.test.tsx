import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { EqCard } from "./EqCard";
import { invokeMock, presets, sent, state } from "./test/helpers";
import type { AppState, EqPreset } from "./types";

function show(s: AppState, list: EqPreset[] = presets) {
  const onPresets = vi.fn();
  render(<EqCard state={s} presets={list} onPresets={onPresets} />);
  return onPresets;
}

const chip = (name: string) => within(screen.getByRole("group", { name: "EQ presets" })).getByRole("button", { name });

beforeEach(() => localStorage.clear());

describe("EqCard: presets", () => {
  it("highlights the preset matching the headset's EQ", () => {
    show(state("connected")); // 0/0/0
    expect(chip("Flat").getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByText("Flat", { selector: ".preset-badge" })).toBeTruthy();
  });

  it("marks custom presets with a star", () => {
    show(state("connected", { eqBands: state("connected").eqBands.map((b, i) => ({ ...b, value: [7, 4, 1][i] })) }));
    expect(chip("★ Commute").getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByText("★ Commute", { selector: ".preset-badge" })).toBeTruthy();
  });

  it("applies a preset's values", async () => {
    show(state("connected"));
    await userEvent.click(chip("Warm"));
    await userEvent.click(chip("Flat")); // already applied: nothing sent
    expect(sent()).toEqual([{ type: "applyEq", args: [4, 0, -3] }]);
  });

  it("calls unsaved values Custom and offers to save them", () => {
    show(state("eqUnsaved")); // 8/4/2
    expect(screen.getByText("Custom", { selector: ".preset-badge" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Save as preset…" })).toBeTruthy();
  });

  it("saves the current values under a name of at most 13 characters", async () => {
    invokeMock.mockImplementation((name: string) => Promise.resolve(name === "save_preset" ? presets : null));
    const onPresets = show(state("eqUnsaved"));
    await userEvent.click(screen.getByRole("button", { name: "Save as preset…" }));
    const input = screen.getByRole("textbox", { name: "Preset name" }) as HTMLInputElement;
    expect(input.maxLength).toBe(13);
    await userEvent.type(input, "Late night{Enter}");
    expect(invokeMock).toHaveBeenCalledWith("save_preset", { name: "Late night", values: [8, 4, 2] });
    await waitFor(() => expect(onPresets).toHaveBeenCalledWith(presets));
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("shows why a name can't be used and keeps the editor open", async () => {
    invokeMock.mockImplementation((name: string) =>
      name === "save_preset" ? Promise.reject('"Bass" is already a preset.') : Promise.resolve(null),
    );
    const onPresets = show(state("eqUnsaved"));
    await userEvent.click(screen.getByRole("button", { name: "Save as preset…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Preset name" }), "Bass");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect((await screen.findByRole("alert")).textContent).toBe('"Bass" is already a preset.');
    expect(onPresets).not.toHaveBeenCalled();
    expect(screen.getByRole("textbox")).toBeTruthy();
  });

  it("can't save a blank name; Esc cancels without closing the flyout", async () => {
    show(state("eqUnsaved"));
    await userEvent.click(screen.getByRole("button", { name: "Save as preset…" }));
    expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(invokeMock).not.toHaveBeenCalledWith("hide_flyout");
  });

  it("renames and deletes the selected custom preset", async () => {
    invokeMock.mockImplementation(() => Promise.resolve(presets));
    const commute = state("connected", { eqBands: state("connected").eqBands.map((b, i) => ({ ...b, value: [7, 4, 1][i] })) });
    show(commute);
    await userEvent.click(screen.getByRole("button", { name: "Rename" }));
    const input = screen.getByRole("textbox", { name: "Preset name" }) as HTMLInputElement;
    expect(input.value).toBe("Commute");
    await userEvent.clear(input);
    await userEvent.type(input, "Trains{Enter}");
    expect(invokeMock).toHaveBeenCalledWith("rename_preset", { id: "custom.commute", name: "Trains" });
    await waitFor(() => expect(screen.queryByRole("textbox")).toBeNull());
    await userEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(invokeMock).toHaveBeenCalledWith("delete_preset", { id: "custom.commute" });
  });

  it("offers no Rename/Delete for built-in presets", () => {
    show(state("connected"));
    expect(screen.queryByRole("button", { name: "Rename" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Save as preset…" })).toBeNull();
  });
});

describe("EqCard: bands and sidetone", () => {
  it("sets one band on release, with signed values", () => {
    show(state("eqUnsaved"));
    const treble = screen.getByLabelText("Treble") as HTMLInputElement;
    expect(treble.value).toBe("2");
    expect(screen.getByText("+8")).toBeTruthy(); // bass
    fireEvent.change(treble, { target: { value: "-4" } });
    expect(screen.getByText("-4")).toBeTruthy();
    fireEvent.keyUp(treble);
    expect(sent()).toEqual([{ type: "setEq", args: { bandId: 2, value: -4 } }]);
  });

  it("disables presets and bands while EQ is being written", () => {
    show(state("eqUnsaved", { busy: "eq:preset" }));
    expect((chip("Flat") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByLabelText("Bass") as HTMLInputElement).disabled).toBe(true);
  });

  it("sets sidetone: Off, High, Med, Low = 0, 1, 2, 3", async () => {
    show(state("eqUnsaved")); // sidetone 2
    const group = screen.getByRole("radiogroup", { name: "Sidetone" });
    expect(within(group).getByRole("radio", { name: "Med" }).getAttribute("aria-checked")).toBe("true");
    await userEvent.click(within(group).getByRole("radio", { name: "Low" }));
    await userEvent.click(within(group).getByRole("radio", { name: "Med" }));
    expect(sent()).toEqual([{ type: "setSidetone", args: 3 }]);
  });

  it("collapses and remembers it", async () => {
    const { unmount } = render(<EqCard state={state("connected")} presets={presets} onPresets={() => {}} />);
    await userEvent.click(screen.getByRole("button", { name: /Equalizer/ }));
    expect(screen.queryByRole("group", { name: "EQ presets" })).toBeNull();
    expect(screen.getByText("Flat", { selector: ".preset-badge" })).toBeTruthy(); // badge stays
    unmount();
    show(state("connected"));
    expect(screen.queryByRole("group", { name: "EQ presets" })).toBeNull();
  });

  it("doesn't call the EQ Custom before the preset list has loaded", () => {
    show(state("connected"), []);
    expect(document.querySelector(".preset-badge")).toBeNull();
    expect(screen.queryByRole("button", { name: "Save as preset…" })).toBeNull();
    expect(screen.getByLabelText("Bass")).toBeTruthy(); // bands work regardless
  });

  it("follows a change made elsewhere, even after the slider was moved", () => {
    const s = state("eqUnsaved"); // treble +2
    const { rerender } = render(<EqCard state={s} presets={presets} onPresets={() => {}} />);
    const treble = () => screen.getByLabelText("Treble") as HTMLInputElement;
    fireEvent.change(treble(), { target: { value: "-4" } }); // dragged, not released
    // BoseBuddy on the Mac sets treble to +6; the next state carries it.
    const elsewhere = { ...s, eqBands: s.eqBands.map((b) => (b.bandId === 2 ? { ...b, value: 6 } : b)) };
    rerender(<EqCard state={elsewhere} presets={presets} onPresets={() => {}} />);
    expect(treble().value).toBe("6");
    expect(screen.getByText("+6")).toBeTruthy();
    fireEvent.blur(treble());
    expect(sent(), "nothing stale written back").toEqual([]);
  });

  it("isn't shown for headphones without EQ or sidetone", () => {
    const { container } = render(
      <EqCard state={state("connected", { supportsEq: false, supportsSidetone: false })} presets={presets} onPresets={() => {}} />,
    );
    expect(container.innerHTML).toBe("");
  });
});
