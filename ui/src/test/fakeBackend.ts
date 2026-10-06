// In-memory Backend for component tests. Tests drive it like the Rust core
// would: resolve commands, emit events, and inspect what the UI sent.
import { vi } from "vitest";
import type { Backend } from "../lib/backend";
import { demoRuneCatalog, demoRunePages } from "../lib/demoRunes";
import type { ImportReportDto } from "../bindings/ImportReportDto";
import type { SettingsDto } from "../bindings/SettingsDto";
import type { StatusDto } from "../bindings/StatusDto";

export const defaultSettings: SettingsDto = {
  importRunes: true,
  importItemSets: true,
  importSummonerSpells: true,
  flashKey: "D",
  replaceCurrentPageWhenFull: false,
  startWithWindows: false,
  startMinimized: false,
};

export const disconnected: StatusDto = { session: "disconnected", summoner: null, patch: null };

export const connected: StatusDto = {
  session: "champSelect",
  summoner: { riotId: "Tester#EUW", level: 312, profileIconUrl: null },
  patch: "16.19.1",
};

export const luxReport: ImportReportDto = {
  championName: "Lux",
  role: "Middle",
  runes: { status: "done", detail: "Rune page \u201cLoL Overlay: Lux\u201d is active" },
  itemSet: { status: "skipped", reason: "Turned off in settings" },
  summonerSpells: { status: "failed", error: "Could not set summoner spells" },
  source: "OP.GG all regions \u00b7 Emerald+ \u00b7 patch 16.19",
  runePages: demoRunePages.slice(0, 3),
  selectedRunePage: 0,
};

export function fakeBackend(initial: { status?: StatusDto; lastImport?: ImportReportDto | null; settings?: SettingsDto } = {}) {
  const statusHandlers = new Set<(s: StatusDto) => void>();
  const importHandlers = new Set<(r: ImportReportDto) => void>();
  let settings = initial.settings ?? defaultSettings;

  const backend = {
    getStatus: vi.fn<Backend["getStatus"]>(async () => initial.status ?? disconnected),
    getLastImport: vi.fn<Backend["getLastImport"]>(async () => initial.lastImport ?? null),
    getSettings: vi.fn<Backend["getSettings"]>(async () => settings),
    saveSettings: vi.fn<Backend["saveSettings"]>(async (next) => {
      settings = next;
      return next;
    }),
    reimport: vi.fn<Backend["reimport"]>(async () => luxReport),
    applyRunePage: vi.fn<Backend["applyRunePage"]>(async (index) => ({ ...luxReport, selectedRunePage: index })),
    getRuneCatalog: vi.fn<Backend["getRuneCatalog"]>(async () => demoRuneCatalog),
    openLogsFolder: vi.fn<Backend["openLogsFolder"]>(async () => undefined),
    appVersion: vi.fn<Backend["appVersion"]>(async () => "0.1.0"),
    onStatus: vi.fn<Backend["onStatus"]>(async (h) => {
      statusHandlers.add(h);
      return () => void statusHandlers.delete(h);
    }),
    onImport: vi.fn<Backend["onImport"]>(async (h) => {
      importHandlers.add(h);
      return () => void importHandlers.delete(h);
    }),
  } satisfies Backend;

  return {
    backend,
    emitStatus: (s: StatusDto) => statusHandlers.forEach((h) => h(s)),
    emitImport: (r: ImportReportDto) => importHandlers.forEach((h) => h(r)),
    listenerCount: () => statusHandlers.size + importHandlers.size,
  };
}
