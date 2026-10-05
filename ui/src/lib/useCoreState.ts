import { useEffect, useState } from "react";
import type { ImportReportDto } from "../bindings/ImportReportDto";
import type { StatusDto } from "../bindings/StatusDto";
import { useBackend } from "./BackendContext";

/**
 * Live status and the latest import report. Subscribes to events first, then
 * fetches the current snapshot, so nothing emitted in between is missed.
 */
export function useCoreState() {
  const backend = useBackend();
  const [status, setStatus] = useState<StatusDto | null>(null);
  const [lastImport, setLastImport] = useState<ImportReportDto | null>(null);

  useEffect(() => {
    let active = true;
    const unlisteners: Array<() => void> = [];
    (async () => {
      const subs = await Promise.all([
        backend.onStatus((s) => active && setStatus(s)),
        backend.onImport((r) => active && setLastImport(r)),
      ]);
      if (!active) return subs.forEach((u) => u());
      unlisteners.push(...subs);
      const [s, r] = await Promise.all([backend.getStatus(), backend.getLastImport()]);
      if (!active) return;
      setStatus((current) => current ?? s);
      setLastImport((current) => current ?? r);
    })().catch((e: unknown) => console.error("could not load core state", e));
    return () => {
      active = false;
      unlisteners.forEach((u) => u());
    };
  }, [backend]);

  return { status, lastImport, setLastImport };
}
