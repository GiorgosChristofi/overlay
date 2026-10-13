import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HomePage } from "./HomePage";
import { connected, fakeBackend } from "../../test/fakeBackend";
import { renderWith } from "../../test/render";

function renderHome() {
  const { backend } = fakeBackend();
  return renderWith(backend, <HomePage status={connected} lastImport={null} setLastImport={() => {}} />);
}

const icon = () => screen.getByRole("button", { name: /Summoner icon/ });

describe("HomePage", () => {
  beforeEach(() => localStorage.clear());

  it("shows the session phase as a single pill", () => {
    renderHome();
    expect(screen.getByRole("status", { name: "Game session: Champ select" })).toHaveTextContent("Champ select");
    expect(screen.queryByRole("list", { name: "Game session" })).not.toBeInTheDocument();
  });

  it("hides the summoner name after three clicks on the icon, and shows it after three more", async () => {
    renderHome();
    expect(screen.getByRole("heading", { name: "Connected as Tester#EUW" })).toBeInTheDocument();

    await userEvent.tripleClick(icon());
    expect(screen.getByRole("heading", { name: "Connected as Hidden" })).toBeInTheDocument();
    expect(screen.queryByText(/Tester#EUW/)).not.toBeInTheDocument();
    expect(icon()).toHaveAttribute("aria-pressed", "true");

    await userEvent.tripleClick(icon());
    expect(screen.getByRole("heading", { name: "Connected as Tester#EUW" })).toBeInTheDocument();
  });

  it("does not hide the name on a single or double click", async () => {
    renderHome();
    await userEvent.click(icon());
    await userEvent.dblClick(icon());
    expect(screen.getByRole("heading", { name: "Connected as Tester#EUW" })).toBeInTheDocument();
  });

  it("keeps the name hidden after a restart", async () => {
    const { unmount } = renderHome();
    await userEvent.tripleClick(icon());
    unmount();
    renderHome();
    expect(screen.getByRole("heading", { name: "Connected as Hidden" })).toBeInTheDocument();
  });

  it("still hides the name when storage is blocked", async () => {
    const get = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    const set = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    renderHome();
    await userEvent.tripleClick(icon());
    expect(screen.getByRole("heading", { name: "Connected as Hidden" })).toBeInTheDocument();
    get.mockRestore();
    set.mockRestore();
  });
});
