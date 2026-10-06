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

  it("shows every step with its own outcome", () => {
    const { backend } = fakeBackend();
    renderWith(backend, <ImportPanel report={luxReport} canReimport={false} onReport={() => {}} />);
    expect(screen.getByRole("img", { name: "Done" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Skipped" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Failed" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Import again" })).not.toBeInTheDocument();
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
