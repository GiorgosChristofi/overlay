import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { RunePicker } from "./RunePicker";
import { fakeBackend, luxReport } from "../../test/fakeBackend";
import { renderWith } from "../../test/render";

describe("RunePicker", () => {
  it("lists every rune page with its statistics and marks the active one", async () => {
    const { backend } = fakeBackend();
    renderWith(backend, <RunePicker report={luxReport} onReport={() => {}} />);
    const pages = await screen.findAllByRole("button", { pressed: false });
    const active = screen.getByRole("button", { pressed: true });
    expect(pages).toHaveLength(2);
    expect(active).toHaveAccessibleName("Arcane Comet, Sorcery and Domination, 51.7% win · 4,492 games");
    expect(screen.getByRole("button", { name: /^Dark Harvest, Domination and Sorcery/ })).toBeInTheDocument();
  });

  it("draws the active page's tree with only the chosen runes named", async () => {
    const { backend } = fakeBackend();
    renderWith(backend, <RunePicker report={luxReport} onReport={() => {}} />);
    const tree = await screen.findByRole("figure", { name: "Active rune page" });
    for (const rune of ["Arcane Comet", "Manaflow Band", "Transcendence", "Scorch", "Cheap Shot", "Ultimate Hunter"]) {
      expect(tree).toContainElement(screen.getByRole("img", { name: rune }));
    }
    expect(screen.queryByRole("img", { name: "Summon Aery" })).not.toBeInTheDocument();
    expect(screen.getAllByRole("img", { name: "Adaptive Force" })).toHaveLength(2);
    expect(screen.getByRole("img", { name: "Health Scaling" })).toBeInTheDocument();
  });

  it("previews a page on hover without sending it", async () => {
    const { backend } = fakeBackend();
    renderWith(backend, <RunePicker report={luxReport} onReport={() => {}} />);
    await userEvent.hover(await screen.findByRole("button", { name: /^Dark Harvest/ }));
    expect(screen.getByRole("figure", { name: "Rune page preview" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Dark Harvest" })).toBeInTheDocument();
    expect(backend.applyRunePage).not.toHaveBeenCalled();
  });

  it("keeps the preview while the pointer moves into its tree, and drops it on leaving", async () => {
    const { backend } = fakeBackend();
    renderWith(backend, <RunePicker report={luxReport} onReport={() => {}} />);
    await userEvent.hover(await screen.findByRole("button", { name: /^Dark Harvest/ }));
    await userEvent.hover(screen.getByRole("img", { name: "Dark Harvest" }));
    expect(screen.getByRole("figure", { name: "Rune page preview" })).toBeInTheDocument();
    await userEvent.unhover(screen.getByRole("region", { name: "Rune pages" }));
    expect(screen.getByRole("figure", { name: "Active rune page" })).toBeInTheDocument();
  });

  it("explains a rune when it is hovered", async () => {
    const { backend } = fakeBackend();
    renderWith(backend, <RunePicker report={luxReport} onReport={() => {}} />);
    const comet = await screen.findByRole("img", { name: "Arcane Comet" });
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    await userEvent.hover(comet);
    const tip = screen.getByRole("tooltip");
    expect(tip).toHaveTextContent("Arcane Comet");
    expect(tip).toHaveTextContent(/comet/i);
    await userEvent.unhover(comet);
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  });

  it("explains stat shards too", async () => {
    const { backend } = fakeBackend();
    renderWith(backend, <RunePicker report={luxReport} onReport={() => {}} />);
    await userEvent.hover(await screen.findByRole("img", { name: "Health Scaling" }));
    expect(screen.getByRole("tooltip")).toHaveTextContent("+10-180 Health (based on level)");
  });

  it("sends the clicked page and reports the result", async () => {
    const { backend } = fakeBackend();
    const onReport = vi.fn();
    renderWith(backend, <RunePicker report={luxReport} onReport={onReport} />);
    await userEvent.click(await screen.findByRole("button", { name: /^Dark Harvest/ }));
    expect(backend.applyRunePage).toHaveBeenCalledWith(1);
    expect(onReport).toHaveBeenCalledWith({ ...luxReport, selectedRunePage: 1 });
  });

  it("does not resend the active page", async () => {
    const { backend } = fakeBackend();
    renderWith(backend, <RunePicker report={luxReport} onReport={() => {}} />);
    await userEvent.click(await screen.findByRole("button", { pressed: true }));
    expect(backend.applyRunePage).not.toHaveBeenCalled();
  });

  it("shows the core's error when the page cannot be sent", async () => {
    const { backend } = fakeBackend();
    backend.applyRunePage.mockRejectedValueOnce({ code: "not_connected", message: "The League client is not running" });
    renderWith(backend, <RunePicker report={luxReport} onReport={() => {}} />);
    await userEvent.click(await screen.findByRole("button", { name: /^Dark Harvest/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("The League client is not running");
  });
});
