import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ImportPanel } from "./ImportPanel";
import { fakeBackend, luxReport } from "../../test/fakeBackend";
import { renderWith } from "../../test/render";

describe("ImportPanel", () => {
  it("invites the player to pick a champion when nothing was imported", () => {
    const { backend } = fakeBackend();
    renderWith(backend, <ImportPanel report={null} canReimport={false} onReport={() => {}} />);
    expect(screen.getByText(/Hover or lock in a champion/)).toBeInTheDocument();
  });

  it("shows only the steps that were skipped or failed, with why", () => {
    const { backend } = fakeBackend();
    renderWith(backend, <ImportPanel report={luxReport} canReimport={false} onReport={() => {}} />);
    expect(screen.queryByRole("img", { name: "Done" })).not.toBeInTheDocument();
    expect(screen.queryByText(/LoL Overlay: Lux/)).not.toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Skipped" })).toBeInTheDocument();
    expect(screen.getByText("Turned off in settings")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Failed" })).toBeInTheDocument();
    expect(screen.getByText("Could not set summoner spells")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Import again" })).not.toBeInTheDocument();
  });

  it("shows no step list or build source when everything was set", () => {
    const { backend } = fakeBackend();
    const allDone = {
      ...luxReport,
      itemSet: { status: "done", detail: "14 items added" },
      summonerSpells: { status: "done", detail: "Flash on D, Teleport on F" },
    } as const;
    renderWith(backend, <ImportPanel report={allDone} canReimport={false} onReport={() => {}} />);
    expect(screen.getByRole("heading", { name: /Lux/ })).toBeInTheDocument();
    expect(screen.queryByRole("list")).not.toBeInTheDocument();
    expect(screen.queryByText(/Build from/)).not.toBeInTheDocument();
  });

  it("imports again and reports the new result", async () => {
    const { backend } = fakeBackend();
    const onReport = vi.fn();
    renderWith(backend, <ImportPanel report={luxReport} canReimport onReport={onReport} />);
    await userEvent.click(screen.getByRole("button", { name: "Import again" }));
    expect(backend.reimport).toHaveBeenCalledOnce();
    expect(onReport).toHaveBeenCalledWith(luxReport);
  });

  it("shows the core's error message when importing again fails", async () => {
    const { backend } = fakeBackend();
    backend.reimport.mockRejectedValueOnce({ code: "not_connected", message: "The League client is not running" });
    renderWith(backend, <ImportPanel report={luxReport} canReimport onReport={() => {}} />);
    await userEvent.click(screen.getByRole("button", { name: "Import again" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("The League client is not running");
  });
});
