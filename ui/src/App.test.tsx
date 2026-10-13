import { act, screen } from "@testing-library/react";
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
    expect(screen.getByRole("status", { name: "Game session: Champ select" })).toBeInTheDocument();
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

  describe("menu", () => {
    beforeEach(() => localStorage.clear());

    const shell = () => document.querySelector(".shell");
    const toggle = (name: string) => screen.getByRole("button", { name });

    it("starts collapsed, with the pages still reachable by name", async () => {
      const { backend } = fakeBackend();
      renderWith(backend, <App />);
      expect(toggle("Expand menu")).toHaveAttribute("aria-expanded", "false");
      expect(shell()).toHaveAttribute("data-rail", "collapsed");
      expect(screen.getByRole("button", { name: "Settings" })).toHaveAttribute("title", "Settings");
      await userEvent.click(screen.getByRole("button", { name: "Settings" }));
      expect(screen.getByRole("button", { name: "Settings" })).toHaveAttribute("aria-current", "page");
    });

    it("expands and collapses, and remembers the choice", async () => {
      const { backend } = fakeBackend();
      const { unmount } = renderWith(backend, <App />);
      await userEvent.click(toggle("Expand menu"));
      expect(toggle("Collapse menu")).toHaveAttribute("aria-expanded", "true");
      expect(shell()).toHaveAttribute("data-rail", "expanded");
      unmount();

      renderWith(backend, <App />);
      expect(shell()).toHaveAttribute("data-rail", "expanded");
      await userEvent.click(toggle("Collapse menu"));
      expect(shell()).toHaveAttribute("data-rail", "collapsed");
    });

    it("still works when storage is blocked", async () => {
      const get = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
        throw new Error("blocked");
      });
      const set = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
        throw new Error("blocked");
      });
      const { backend } = fakeBackend();
      renderWith(backend, <App />);
      expect(shell()).toHaveAttribute("data-rail", "collapsed");
      await userEvent.click(toggle("Expand menu"));
      expect(shell()).toHaveAttribute("data-rail", "expanded");
      get.mockRestore();
      set.mockRestore();
    });
  });

  it("navigates between pages", async () => {
    const { backend } = fakeBackend();
    renderWith(backend, <App />);
    await userEvent.click(screen.getByRole("button", { name: "About" }));
    expect(await screen.findByText(/is not endorsed by Riot Games/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "About" })).toHaveAttribute("aria-current", "page");
  });
});
