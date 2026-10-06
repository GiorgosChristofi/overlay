// The only file that talks to Tauri. Components use the `Backend` interface,
// so tests swap in a fake without mocking Tauri internals.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppErrorDto } from "../bindings/AppErrorDto";
import type { ImportReportDto } from "../bindings/ImportReportDto";
import type { RuneCatalogDto } from "../bindings/RuneCatalogDto";
import type { SettingsDto } from "../bindings/SettingsDto";
import type { StatusDto } from "../bindings/StatusDto";

// Mirrors ipc_types::EVENT_STATUS / EVENT_IMPORT.
export const EVENT_STATUS = "core://status";
export const EVENT_IMPORT = "core://import";

export type Unlisten = UnlistenFn;

export interface Backend {
  getStatus(): Promise<StatusDto>;
  getLastImport(): Promise<ImportReportDto | null>;
  getSettings(): Promise<SettingsDto>;
  saveSettings(settings: SettingsDto): Promise<SettingsDto>;
  reimport(): Promise<ImportReportDto>;
  /** Sends rune page `index` of the last import's `runePages` to the client. */
  applyRunePage(index: number): Promise<ImportReportDto>;
  /** The current patch's rune trees; `null` while game data loads. */
  getRuneCatalog(): Promise<RuneCatalogDto | null>;
  openLogsFolder(): Promise<void>;
  appVersion(): Promise<string>;
  onStatus(handler: (status: StatusDto) => void): Promise<Unlisten>;
  onImport(handler: (report: ImportReportDto) => void): Promise<Unlisten>;
}

/** Normalises anything thrown by a command into an `AppErrorDto`. */
export function toAppError(error: unknown): AppErrorDto {
  if (typeof error === "object" && error !== null && "code" in error && "message" in error) {
    return error as AppErrorDto;
  }
  return { code: "unknown", message: String(error) };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toAppError(error);
  }
}

export const tauriBackend: Backend = {
  getStatus: () => call("get_status"),
  getLastImport: () => call("get_last_import"),
  getSettings: () => call("get_settings"),
  saveSettings: (settings) => call("save_settings", { settings }),
  reimport: () => call("reimport"),
  applyRunePage: (index) => call("apply_rune_page", { index }),
  getRuneCatalog: () => call("get_rune_catalog"),
  openLogsFolder: () => call("open_logs_folder"),
  appVersion: () => call("app_version"),
  onStatus: (handler) => listen<StatusDto>(EVENT_STATUS, (e) => handler(e.payload)),
  onImport: (handler) => listen<ImportReportDto>(EVENT_IMPORT, (e) => handler(e.payload)),
};
