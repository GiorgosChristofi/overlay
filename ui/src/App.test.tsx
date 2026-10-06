import { act, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "./App";
import { connected, fakeBackend, luxReport } from "./test/fakeBackend";
import { renderWith } from "./test/render";

describe("App", () => {
  it("waits for the client when disconnected", async () => {
    const { backend } = fakeBackend();
    renderWith(backend, <App />);
    expect(await screen.findByRole("heading", { name: "Waiting for the League client" })).toBeInTheDocument();
    expect(screen.getByText("Client not found")).toBeInTheDocument();
  });

  it("updates live when the core emits a new status", async () => {
    const fake = fakeBackend();
    renderWith(fake.backend, <App />);
    await screen.findByText("Waiting for the League client");

    act(() => fake.emitStatus(connected));

    expect(await screen.findByRole("heading", { name: "Connected as Tester#EUW" })).toBeInTheDocument();
    expect(screen.getByText("Level 312")).toBeInTheDocument();
    expect(screen.getByText("Patch 16.19.1")).toBeInTheDocument();
    const track = screen.getByRole("list", { name: "Game session" });
    expect(within(track).getByText("Champ select").closest("li")).toHaveAttribute("aria-current", "step");
  });

  it("shows an import as soon as it is emitted", async () => {
    const fake = fakeBackend({ status: connected });
    renderWith(fake.backend, <App />);
    await screen.findByText(/Hover or lock in a champion/);

    act(() => fake.emitImport(luxReport));

    expect(await screen.findByRole("heading", { name: /Lux/ })).toBeInTheDocument();
    expect(screen.getByText("Turned off in settings")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Failed" })).toBeInTheDocument();
  });

  it("unsubscribes from events on unmount", async () => {
    const fake = fakeBackend();
    const { unmount } = renderWith(fake.backend, <App />);
    await screen.findByText("Waiting for the League client");
    expect(fake.listenerCount()).toBe(2);
    unmount();
    expect(fake.listenerCount()).toBe(0);
  });

  it("navigates between pages", async () => {
    const { backend } = fakeBackend();
    renderWith(backend, <App />);
    await userEvent.click(screen.getByRole("button", { name: "About" }));
    expect(await screen.findByText(/is not endorsed by Riot Games/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "About" })).toHaveAttribute("aria-current", "page");
  });
});
