import { useEffect, useId, useState } from "react";
import type { SettingsDto } from "../../bindings/SettingsDto";
import { useBackend } from "../../lib/BackendContext";

type BoolKey = { [K in keyof SettingsDto]: SettingsDto[K] extends boolean ? K : never }[keyof SettingsDto];

function Toggle({ label, hint, checked, onChange }: { label: string; hint?: string; checked: boolean; onChange: (v: boolean) => void }) {
  const id = useId();
  return (
    <div className="setting">
      <div className="setting-text">
        <label htmlFor={id}>{label}</label>
        {hint && <p className="setting-hint">{hint}</p>}
      </div>
      <input id={id} type="checkbox" role="switch" className="switch" checked={checked} onChange={(e) => onChange(e.target.checked)} />
    </div>
  );
}

export function SettingsPage() {
  const backend = useBackend();
  const [settings, setSettings] = useState<SettingsDto | null>(null);
  const [saveState, setSaveState] = useState<{ kind: "idle" | "saved" } | { kind: "error"; message: string }>({ kind: "idle" });

  useEffect(() => {
    backend.getSettings().then(setSettings, (e: unknown) => console.error("could not load settings", e));
  }, [backend]);

  if (!settings) return <p className="loading">Loading settings…</p>;

  async function update(next: SettingsDto) {
    const previous = settings;
    setSettings(next);
    try {
      setSettings(await backend.saveSettings(next));
      setSaveState({ kind: "saved" });
    } catch (e) {
      setSettings(previous);
      setSaveState({ kind: "error", message: (e as { message?: string }).message ?? "Could not save settings" });
    }
  }

  const toggle = (key: BoolKey) => (value: boolean) => update({ ...settings, [key]: value });

  return (
    <>
      <header className="page-head">
        <h1>Settings</h1>
        <p className="save-state" role="status" data-kind={saveState.kind}>
          {saveState.kind === "saved" && "Saved"}
          {saveState.kind === "error" && saveState.message}
        </p>
      </header>

      <section className="settings-group" aria-labelledby="cs-title">
        <h2 id="cs-title">Champion select</h2>
        <Toggle label="Import rune pages" checked={settings.importRunes} onChange={toggle("importRunes")} />
        <Toggle label="Import item sets" hint="Shown in the shop's recommended tab." checked={settings.importItemSets} onChange={toggle("importItemSets")} />
        <Toggle label="Set summoner spells" checked={settings.importSummonerSpells} onChange={toggle("importSummonerSpells")} />
        <div className="setting">
          <div className="setting-text">
            <span id="flash-label">Flash key</span>
          </div>
          <div className="segmented" role="radiogroup" aria-labelledby="flash-label">
            {(["D", "F"] as const).map((key) => (
              <button
                key={key}
                type="button"
                role="radio"
                aria-checked={settings.flashKey === key}
                onClick={() => settings.flashKey !== key && update({ ...settings, flashKey: key })}
              >
                {key}
              </button>
            ))}
          </div>
        </div>
        <Toggle
          label="Replace my current rune page when all pages are in use"
          hint="Off: the import skips runes instead of touching your pages."
          checked={settings.replaceCurrentPageWhenFull}
          onChange={toggle("replaceCurrentPageWhenFull")}
        />
      </section>

      <section className="settings-group" aria-labelledby="game-title">
        <h2 id="game-title">In game</h2>
        <Toggle
          label="Show the item column when I press P"
          hint="What to build next and what it costs, over the game. Needs League in Borderless or Windowed mode."
          checked={settings.overlayEnabled}
          onChange={toggle("overlayEnabled")}
        />
      </section>

      <section className="settings-group" aria-labelledby="startup-title">
        <h2 id="startup-title">Startup</h2>
        <Toggle label="Start with Windows" checked={settings.startWithWindows} onChange={toggle("startWithWindows")} />
        <Toggle label="Start minimized to the tray" checked={settings.startMinimized} onChange={toggle("startMinimized")} />
      </section>
    </>
  );
}
