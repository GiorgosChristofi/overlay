import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SettingsPage } from "./SettingsPage";
import { defaultSettings, fakeBackend } from "../../test/fakeBackend";
import { renderWith } from "../../test/render";

describe("SettingsPage", () => {
  it("saves a toggle immediately and confirms", async () => {
    const { backend } = fakeBackend();
    renderWith(backend, <SettingsPage />);
    await userEvent.click(await screen.findByRole("switch", { name: "Import item sets" }));
    expect(backend.saveSettings).toHaveBeenCalledWith({ ...defaultSettings, importItemSets: false });
    expect(await screen.findByRole("status")).toHaveTextContent("Saved");
  });

  it("turns the in-game item column off", async () => {
    const { backend } = fakeBackend();
    renderWith(backend, <SettingsPage />);
    const toggle = await screen.findByRole("switch", { name: "Show the item column when I press P" });
    expect(toggle).toBeChecked();
    await userEvent.click(toggle);
    expect(backend.saveSettings).toHaveBeenCalledWith({ ...defaultSettings, overlayEnabled: false });
  });

  it("switches the Flash key", async () => {
    const { backend } = fakeBackend();
    renderWith(backend, <SettingsPage />);
    await userEvent.click(await screen.findByRole("radio", { name: "F" }));
    expect(backend.saveSettings).toHaveBeenCalledWith({ ...defaultSettings, flashKey: "F" });
    expect(screen.getByRole("radio", { name: "F" })).toHaveAttribute("aria-checked", "true");
  });

  it("reverts the switch and explains when saving fails", async () => {
    const { backend } = fakeBackend();
    backend.saveSettings.mockRejectedValueOnce({ code: "autostart", message: "Windows refused to change the start-up entry" });
    renderWith(backend, <SettingsPage />);
    const toggle = await screen.findByRole("switch", { name: "Start with Windows" });
    await userEvent.click(toggle);
    expect(await screen.findByRole("status")).toHaveTextContent("Windows refused");
    expect(toggle).not.toBeChecked();
  });
});
