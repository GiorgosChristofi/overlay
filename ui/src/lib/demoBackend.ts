// Sample data for working on the UI in a normal browser (`npm run dev`),
// without Tauri or League running. `?state=disconnected|lobby|champSelect|empty`
// picks the scene; `&page=N` the active rune page.
import type { Backend } from "./backend";
import { demoRuneCatalog, demoRunePages } from "./demoRunes";
import type { ImportReportDto } from "../bindings/ImportReportDto";
import type { SessionStateDto } from "../bindings/SessionStateDto";
import type { SettingsDto } from "../bindings/SettingsDto";
import type { StatusDto } from "../bindings/StatusDto";

const params = new URLSearchParams(window.location.search);
const scene = params.get("state") ?? "champSelect";

const status: StatusDto =
  scene === "disconnected"
    ? { session: "disconnected", summoner: null, patch: null }
    : {
        session: (scene === "empty" ? "champSelect" : scene) as SessionStateDto,
        summoner: { riotId: "Demo Player#EUNE", level: 287, profileIconUrl: null },
        patch: "16.19.1",
      };

let report: ImportReportDto | null =
  scene === "champSelect"
    ? {
        championName: "Lux",
        role: "Middle",
        runes: { status: "done", detail: "Rune page “LoL Overlay: Lux” is active" },
        itemSet: { status: "done", detail: "13 items added to the shop's recommended tab" },
        summonerSpells: { status: "skipped", reason: "Turned off in settings" },
        source: "OP.GG all regions · Emerald+ · patch 16.19",
        runePages: demoRunePages,
        selectedRunePage: Number(params.get("page") ?? 0) % demoRunePages.length,
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
  overlayEnabled: true,
};

const noop = async () => () => {};
const nothingToImport = { code: "nothing_to_import", message: "Pick a champion first" };

export const demoBackend: Backend = {
  getStatus: async () => status,
  getLastImport: async () => report,
  getSettings: async () => settings,
  saveSettings: async (next) => (settings = next),
  reimport: async () => {
    if (!report) throw nothingToImport;
    return report;
  },
  applyRunePage: async (index) => {
    if (!report) throw nothingToImport;
    await new Promise((resolve) => setTimeout(resolve, 250));
    report = { ...report, selectedRunePage: index };
    return report;
  },
  getRuneCatalog: async () => demoRuneCatalog,
  openLogsFolder: async () => undefined,
  appVersion: async () => "0.1.0-demo",
  onStatus: noop,
  onImport: noop,
};
