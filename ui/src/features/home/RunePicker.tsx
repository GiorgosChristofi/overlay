import { useEffect, useId, useState, type FocusEvent } from "react";
import type { ImportReportDto } from "../../bindings/ImportReportDto";
import type { RuneCatalogDto } from "../../bindings/RuneCatalogDto";
import type { RuneDto } from "../../bindings/RuneDto";
import type { RunePageDto } from "../../bindings/RunePageDto";
import type { RuneTreeDto } from "../../bindings/RuneTreeDto";
import { useBackend } from "../../lib/BackendContext";

const percent = (rate: number) => `${(rate * 100).toFixed(1)}%`;

function statsText(page: RunePageDto): string | null {
  const parts: string[] = [];
  if (page.winRate != null) parts.push(`${percent(page.winRate)} win`);
  if (page.games != null) parts.push(`${page.games.toLocaleString("en-US")} games`);
  return parts.length ? parts.join(" · ") : null;
}

function findRune(catalog: RuneCatalogDto, id: number): RuneDto | undefined {
  for (const tree of catalog.trees) {
    for (const row of tree.slots) {
      const rune = row.find((r) => r.id === id);
      if (rune) return rune;
    }
  }
  return undefined;
}

/**
 * A rune icon that explains itself on hover. Chosen runes can also be
 * reached with the keyboard; the rest of the tree is mouse-only context.
 */
function RuneIcon({ rune, chosen, size }: { rune: RuneDto; chosen: boolean; size: number }) {
  const [open, setOpen] = useState(false);
  const tipId = useId();
  return (
    <span
      className="rune"
      data-chosen={chosen}
      style={{ width: size, height: size }}
      tabIndex={chosen ? 0 : undefined}
      aria-describedby={open ? tipId : undefined}
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      onFocus={() => setOpen(true)}
      onBlur={() => setOpen(false)}
    >
      {rune.iconUrl ? (
        // Only chosen runes are announced; the rest of the tree is context.
        <img src={rune.iconUrl} alt={chosen ? rune.name : ""} width={size} height={size} />
      ) : (
        <span className="rune-fallback" role={chosen ? "img" : undefined} aria-label={chosen ? rune.name : undefined}>
          {rune.name.slice(0, 2)}
        </span>
      )}
      {open && (
        <span className="rune-tip" role="tooltip" id={tipId}>
          <strong className="rune-tip-name">{rune.name}</strong>
          {rune.description && <span className="rune-tip-text">{rune.description}</span>}
        </span>
      )}
    </span>
  );
}

function TreeColumn({ tree, rows, chosen, label }: { tree: RuneTreeDto; rows: RuneDto[][]; chosen: number[]; label: string }) {
  return (
    <div className="rune-tree">
      <p className="rune-tree-head">
        {tree.iconUrl && <img src={tree.iconUrl} alt="" width={20} height={20} />}
        <span>
          {tree.name} <span className="rune-tree-kind">{label}</span>
        </span>
      </p>
      {rows.map((row, i) => (
        <div key={i} className="rune-row" data-keystone={label === "Primary" && i === 0}>
          {row.map((rune) => (
            <RuneIcon key={rune.id} rune={rune} chosen={chosen.includes(rune.id)} size={label === "Primary" && i === 0 ? 40 : 30} />
          ))}
        </div>
      ))}
    </div>
  );
}

/** One page drawn like the client's rune editor: both trees and the shards. */
export function RunePageView({ page, catalog }: { page: RunePageDto; catalog: RuneCatalogDto }) {
  const primary = catalog.trees.find((t) => t.id === page.primaryStyleId);
  const secondary = catalog.trees.find((t) => t.id === page.subStyleId);
  if (!primary || !secondary) {
    return <p className="rune-missing">This page uses a rune tree missing from the current patch.</p>;
  }
  const shards = page.perkIds.slice(6);
  return (
    <div className="rune-page">
      <TreeColumn tree={primary} rows={primary.slots} chosen={page.perkIds.slice(0, 4)} label="Primary" />
      <TreeColumn tree={secondary} rows={secondary.slots.slice(1)} chosen={page.perkIds.slice(4, 6)} label="Secondary" />
      <div className="rune-tree">
        <p className="rune-tree-head">
          <span>Stat shards</span>
        </p>
        {catalog.statShardRows.map((row, i) => (
          <div key={i} className="rune-row rune-row-shards">
            {/* The same shard can sit in several rows, so match by row. */}
            {row.map((rune) => (
              <RuneIcon key={rune.id} rune={rune} chosen={shards[i] === rune.id} size={26} />
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

interface Props {
  report: ImportReportDto;
  onReport: (report: ImportReportDto) => void;
}

/**
 * The build's rune pages. Hovering or focusing one shows its full tree;
 * clicking sends it to the client.
 */
export function RunePicker({ report, onReport }: Props) {
  const backend = useBackend();
  const [catalog, setCatalog] = useState<RuneCatalogDto | null>(null);
  const [preview, setPreview] = useState<number | null>(null);
  const [applying, setApplying] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    backend
      .getRuneCatalog()
      .then((c) => active && setCatalog(c))
      .catch((e: unknown) => console.error("could not load rune trees", e));
    return () => {
      active = false;
    };
  }, [backend]);

  const pages = report.runePages;
  const selected = report.selectedRunePage;

  async function choose(index: number) {
    if (index === selected || applying != null) return;
    setApplying(index);
    setError(null);
    try {
      onReport(await backend.applyRunePage(index));
    } catch (e) {
      setError((e as { message?: string }).message ?? "Could not change the rune page");
    } finally {
      setApplying(null);
    }
  }

  if (!catalog) {
    return <p className="rune-missing">Loading rune trees…</p>;
  }

  const shown = preview ?? selected ?? 0;
  const shownPage = pages[shown];
  // The preview stays while the pointer or focus moves from a page down to
  // its tree, so its runes can be inspected; it ends on leaving the section.
  const leaveSection = (e: FocusEvent<HTMLElement>) => {
    if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setPreview(null);
  };
  return (
    <section className="runes" aria-labelledby="runes-title" onMouseLeave={() => setPreview(null)} onBlur={leaveSection}>
      <div className="runes-head">
        <h3 id="runes-title">Rune pages</h3>
        <p className="runes-hint">Hover a page to inspect it, a rune to see what it does; click a page to use it</p>
      </div>
      <ul className="rune-options">
        {pages.map((page, i) => {
          const keystone = findRune(catalog, page.perkIds[0] ?? 0);
          const primary = catalog.trees.find((t) => t.id === page.primaryStyleId);
          const secondary = catalog.trees.find((t) => t.id === page.subStyleId);
          const stats = statsText(page);
          const name = keystone?.name ?? `Page ${i + 1}`;
          return (
            <li key={i}>
              <button
                type="button"
                className="rune-option"
                aria-pressed={i === selected}
                aria-busy={applying === i}
                aria-label={`${name}, ${primary?.name ?? "?"} and ${secondary?.name ?? "?"}${stats ? `, ${stats}` : ""}`}
                data-previewed={i === shown}
                disabled={applying != null && applying !== i}
                onMouseEnter={() => setPreview(i)}
                onFocus={() => setPreview(i)}
                onClick={() => void choose(i)}
              >
                <span className="rune-option-icons" aria-hidden="true">
                  {keystone?.iconUrl && <img src={keystone.iconUrl} alt="" width={36} height={36} />}
                  {secondary?.iconUrl && <img className="rune-option-sub" src={secondary.iconUrl} alt="" width={18} height={18} />}
                </span>
                <span className="rune-option-text" aria-hidden="true">
                  <span className="rune-option-name">{name}</span>
                  <span className="rune-option-trees">
                    {primary?.name} + {secondary?.name}
                  </span>
                  {stats && <span className="rune-option-stats">{stats}</span>}
                  {page.pickRate != null && <span className="rune-option-stats">{percent(page.pickRate)} pick rate</span>}
                </span>
                {i === selected && <span className="rune-option-badge">Active</span>}
                {applying === i && <span className="rune-option-badge">Applying…</span>}
              </button>
            </li>
          );
        })}
      </ul>
      {shownPage && (
        <figure className="rune-preview" aria-label={shown === selected ? "Active rune page" : "Rune page preview"}>
          <RunePageView page={shownPage} catalog={catalog} />
        </figure>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
