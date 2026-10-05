// Sample data for working on the UI in a normal browser (`npm run dev`),
// without Tauri or League running. `?state=disconnected|lobby|champSelect|empty`
// picks the scene.
import type { Backend } from "./backend";
import type { ImportReportDto } from "../bindings/ImportReportDto";
import type { SessionStateDto } from "../bindings/SessionStateDto";
import type { SettingsDto } from "../bindings/SettingsDto";
import type { StatusDto } from "../bindings/StatusDto";

const scene = new URLSearchParams(window.location.search).get("state") ?? "champSelect";

const status: StatusDto =
  scene === "disconnected"
    ? { session: "disconnected", summoner: null, patch: null }
    : {
        session: (scene === "empty" ? "champSelect" : scene) as SessionStateDto,
        summoner: { riotId: "Demo Player#EUW", level: 287, profileIconUrl: null },
        patch: "16.19.1",
      };

const report: ImportReportDto | null =
  scene === "champSelect"
    ? {
        championName: "Lux",
        role: "Middle",
        runes: { status: "done", detail: "Rune page \u201cLoL Overlay: Lux\u201d is active" },
        itemSet: { status: "done", detail: "5 items added; 2 removed this patch were left out" },
        summonerSpells: { status: "skipped", reason: "Turned off in settings" },
      }
    : null;

let settings: SettingsDto = {
  importRunes: true,
  importItemSets: true,
  importSummonerSpells: false,
  flashKey: "D",
  replaceCurrentPageWhenFull: false,
  startWithWindows: true,
  startMinimized: false,
};

const noop = async () => () => {};

export const demoBackend: Backend = {
  getStatus: async () => status,
  getLastImport: async () => report,
  getSettings: async () => settings,
  saveSettings: async (next) => (settings = next),
  reimport: async () => {
    if (!report) throw { code: "nothing_to_import", message: "Lock in a champion first" };
    return report;
  },
  openLogsFolder: async () => undefined,
  appVersion: async () => "0.1.0-demo",
  onStatus: noop,
  onImport: noop,
};
