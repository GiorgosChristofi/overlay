# Notes for agents working on LoL Overlay

Read README.md first (product, layout, commands, OP.GG and overlay docs).
This file is what the README doesn't say: how things fit, conventions, and
traps in this Windows dev environment.

## Shape of the code
- `domain` is pure (no I/O). Ports in `crates/domain/src/ports.rs`:
  `LcuPort` (League client), `BuildSource` (OP.GG, seed), `LiveGamePort`
  (in-game Live Client Data API). `app-core::Core` only sees ports; tests
  use fakes (`crates/app-core/tests/import_flow.rs` has FakeLcu, FakeSource,
  RecordingSink).
- Build pipeline: `opgg` adapter (undocumented `lol-api-champion.op.gg`
  JSON, region `global`, Emerald+) -> `domain::builds::Build` -> Core
  validates rune pages against the patch (`static-data`, Data Dragon) ->
  `crates/app-core/src/import.rs` applies runes / item set / spells via LCU.
  Seed builds (`data/builds/seed.json`) are only the offline fallback.
- Import runs on hover *or* lock (`champ_select::selected_pick`), once per
  `ChampionPick`.
- In-game column: `apps/overlay` (egui) runs as a second process,
  `desktop.exe --overlay` (branch in `apps/desktop/src/main.rs`).
  Desktop -> overlay: `ipc_types::OverlayMessage` JSON lines on stdin.
  Overlay -> desktop: `OverlayCommand` JSON lines on stdout
  (`apps/desktop/src/overlay_host.rs`). The view is computed in
  `crates/app-core/src/overlay.rs` from build + purchases + choices + live
  snapshot; shopping rules (bought items, starter phase, next item, boots
  slot, recipe and gold) are pure in `crates/domain/src/shopping.rs`.
- Live data: `crates/lcu-client/src/live.rs` polls
  `/liveclientdata/allgamedata` once a second while in game
  (`apps/desktop/src/lib.rs::spawn_live_game`). The API is HTTP only (no
  websocket or push); see Open threads.
- IPC DTOs live in `ipc-types`; `cargo test` regenerates `ui/src/bindings`
  (ts-rs). CI fails if bindings are stale, so commit them. Overlay DTOs are
  serde-only (no TypeScript).

## Conventions
- Workspace lints: `unsafe_code = forbid`; `unwrap/expect/panic` denied
  outside tests (test files start with an `#![allow(...)]`);
  `print_stdout` warns (use `eprintln!` or write to a locked stdout).
  Always run `cargo clippy --workspace --all-targets -- -D warnings`.
- Parse Riot and OP.GG payloads tolerantly: `#[serde(default)]` everywhere;
  OP.GG also sends `null` (see `nullable` in `crates/opgg/src/parse.rs`).
- Comments explain why; tests are named as behaviours. Match that.
- Never put a Claude co-author trailer on commits of code Claude didn't
  write; the user's settings disable attribution anyway. The user usually
  commits themselves; when asked, give a commit message.

## Commands
- `cargo test` (default members = `crates/*`), `cargo test -p overlay`
  (Windows), `cd ui && npm test && npm run typecheck && npm run lint`.
- Live checks: `cargo test -p opgg -- --ignored`,
  `cargo test -p static-data --test seed_builds -- --ignored`.
- Toolchain pinned to Rust 1.99.0 (`rust-toolchain.toml` and CI). egui and
  eframe 0.36 need >= 1.95. `cargo-deny` is not installed locally (CI runs
  it); `deny.toml` limits targets and excepts egui's font licences.

## Trying the overlay without a game
- `cargo build -p desktop`, then pipe a JSON line into it:
  `(cat view.jsonl; ping -n 15 127.0.0.1 >/dev/null) | LOL_OVERLAY_OPEN=1 LOL_OVERLAY_ANY_WINDOW=1 ./target/debug/desktop.exe --overlay`
  (`OPEN` starts expanded; `ANY_WINDOW` treats any focused window as the
  game). Without `OPEN` it shows the compact strip (needs `gold` set).
  The message must include every `OverlayViewDto` field, or it is ignored.
- Screenshots: find the window titled `LoL Overlay items` with Win32
  `EnumWindows` + `GetWindowRect` from PowerShell and `CopyFromScreen`
  that rect only (don't capture the user's whole screen). The user has 3
  monitors; the primary is at (0,0).
- egui remembers the position in `%APPDATA%\LoL Overlay items\data\app.ron`;
  delete it to test first-run placement. The user may drag the window
  while you test.
- UI demo without Tauri: `cd ui && npm run dev`, then
  `?state=champSelect&page=N`.

## Environment traps (Windows, Git Bash)
- Inline `python - <<'EOF'` heredocs sometimes break when the script
  contains an apostrophe; write the script to the scratchpad and run it.
- `cargo fmt` rewrites files; re-read before string-matching edits.
- `/tmp` in Git Bash is not Python's `/tmp`; use the scratchpad path.
- Background `vite preview` or other servers survive TaskStop as a child
  `node.exe`; free the port with `netstat -ano` + `taskkill //PID <pid> //T //F`
  (confirm the process is ours first).
- Windows takes about 2 s to refuse a closed local port; the Live client
  uses a 1 s connect timeout so "no game" is answered fast.

## Facts that cost time to find
- No API exposes whether the in-game shop is open (Live Client Data has no
  UI state); the column follows P and Esc presses. Don't add screen reading.
- The Live Client API is read-only; nothing can buy items. Automating input
  breaks Riot's terms (Vanguard).
- Free items (trinket) must not end the starter phase; consumed potions and
  merged components still count as bought (`Purchases` = ever seen).
- Overlay window: never hidden at the OS level (showing it again can steal
  game focus); "hidden" means draw nothing + mouse passthrough. It is laid
  out at full height in a `scope_builder` and resized once, otherwise it
  "slides" open. Dragging starts on mouse press (`StartDrag`), not after
  egui's drag threshold.
- eframe shows its window after the first frame and Windows places it then,
  so first-run placement retries `OuterPosition` briefly.

## Not verified yet
- No agent has run anything against a real League client or game; the user
  tests in real games. LCU paths and Live Client payloads come from docs and
  synthetic fixtures (`fixtures/synthetic/`).

## Open threads (offered, not done)
- Live polling: switch from `allgamedata` (grows with events) to
  `/liveclientdata/activeplayer` + `/liveclientdata/playeritems?riotId=`;
  optionally poll faster while the column is open.
- OP.GG region and tier as user settings (now constants in
  `apps/desktop/src/lib.rs` and `crates/opgg/src/lib.rs`).
- Known limits: typing "p" in chat toggles the column; items sold back
  still count as bought.
- `docs/adr/Changes_todo.md` (runes on hover) is done but left untouched.
