import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NoiseCard } from "./NoiseCard";
import { sent, state } from "./test/helpers";

const slider = () => screen.getByLabelText("Noise cancelling") as HTMLInputElement;

describe("NoiseCard: editing", () => {
  it("offers + Custom when a slot is free, and creates it", async () => {
    render(<NoiseCard state={state("connected")} />);
    await userEvent.click(screen.getByRole("button", { name: "+ Custom" }));
    expect(sent()).toEqual([{ type: "createCustomMode" }]);
  });

  it("hides + Custom when no slot is free", () => {
    render(<NoiseCard state={state("customMode")} />);
    expect(screen.queryByRole("button", { name: "+ Custom" })).toBeNull();
  });

  it("doesn't offer editing for Quiet and Aware", () => {
    render(<NoiseCard state={state("connected")} />);
    expect(screen.queryByLabelText("Noise cancelling")).toBeNull();
    expect(screen.queryByRole("switch", { name: "Wind Block" })).toBeNull();
  });

  it("edits the selected user mode's noise cancelling, sent once on release", () => {
    // Custom was created from Quiet: raw CNC 0 = ANC 10.
    render(<NoiseCard state={state("customMode")} />);
    expect(slider().value).toBe("10");
    fireEvent.change(slider(), { target: { value: "8" } });
    fireEvent.change(slider(), { target: { value: "7" } });
    expect(sent(), "nothing while dragging").toEqual([]);
    expect(screen.getByText("7")).toBeTruthy();
    fireEvent.pointerUp(slider());
    fireEvent.blur(slider());
    // ANC 7 is stored as raw CNC 3.
    expect(sent()).toEqual([{ type: "updateMode", args: { index: 3, rawCnc: 3, windBlock: false } }]);
  });

  it("doesn't write when released where it started", () => {
    render(<NoiseCard state={state("customMode")} />);
    fireEvent.pointerUp(slider());
    fireEvent.keyUp(slider());
    expect(sent()).toEqual([]);
  });

  it("turns Wind Block on and greys out the slider", async () => {
    render(<NoiseCard state={state("customMode")} />);
    await userEvent.click(screen.getByRole("switch", { name: "Wind Block" }));
    expect(sent()).toEqual([{ type: "updateMode", args: { index: 3, rawCnc: 0, windBlock: true } }]);
  });

  it("shows Auto under Wind Block and turns it off at full ANC", async () => {
    render(<NoiseCard state={state("lowBattery")} />); // Walk, Wind Block on
    expect(slider().disabled).toBe(true);
    expect(screen.getByText("Auto")).toBeTruthy();
    await userEvent.click(screen.getByRole("switch", { name: "Wind Block" }));
    expect(sent()).toEqual([{ type: "updateMode", args: { index: 2, rawCnc: 0, windBlock: false } }]);
  });

  it("says what it's doing and blocks other changes meanwhile", () => {
    render(<NoiseCard state={state("customMode", { busy: "mode-edit" })} />);
    expect(screen.getByText("Saving to headphones…")).toBeTruthy();
    expect(slider().disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Quiet" }) as HTMLButtonElement).disabled).toBe(true);
  });
});
