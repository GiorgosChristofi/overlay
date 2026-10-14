# Notes for agents working on LoL Overlay

Read README.md first (product, layout, commands, OP.GG and overlay docs).
This file is what the README doesn't say: how things fit, conventions, and
traps in this Windows dev environment.

## Workflow (required, from the user)
- Never commit to `main`. Work on a feature branch (`feat/...`, `fix/...`)
  and push that branch to `origin` (SSH deploy key, already set up). The
  user reviews and merges.
- Every change is tested two ways before it is pushed, and the summary says
  how:
  1. **It works:** unit tests for the logic, plus a `mock-league` end-to-end
     test (`crates/mock-league/tests/end_to_end.rs`) when the change touches
     the client, the game or the import/overlay flow. Run the real app
     against the mock (below) when the change is visible in the UI or overlay.
  2. **It is safe:** tests for the security properties the change touches
     (TLS trust, `riot:<token>` auth, only expected LCU writes, never deleting
     the player's data without consent, bounds on untrusted input, paths
     built from remote data). New dependencies: crates.io / npmjs only,
     checked against OSV (`api.osv.dev/v1/querybatch`, max 1000 per call)
     since `cargo-deny` and `cargo-audit` are not installed.
- Then `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, and the UI checks if `ui/` changed.

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
- Objective timers: rules pure in `crates/domain/src/objectives.rs` (game
  clock + `DragonKill`/`BaronKill` events, timings as constants); Core
  sends `OverlayMessage::Objectives` separately from the item view, every
  read while on the Rift and `None` once after. The overlay draws them in a
  second egui viewport (`apps/overlay/src/minimap_view.rs`, window title
  `LoL Overlay timers`), counting on from the last game time for at most
  1.5 s (`minimap.rs`), so a stalled read never runs a timer ahead. Timed
  modes: `CLASSIC` and `PRACTICETOOL` on map 11 (`live.rs`; the mode is
  logged once per game). A clock that steps back >5 s is a Practice Tool
  reset: kills seen before it no longer count (the game may keep or clear
  its event list) and purchases start over. Scuttlers get only their
  fixed first spawn (2:55; their spots in `minimap.rs` are community
  coordinates); overlapping labels are nudged apart (`minimap_view.rs`).
  Other normal camps cannot be timed: the public API does not report them; Porofessor
  gets them from Overwolf's own `jungle_camps` game events.
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
- Dependency watch on GitHub: cargo-deny (`ci.yml`), `npm audit` +
  `npm audit signatures` and PR dependency review (`dependencies.yml`),
  nightly and on every PR; Dependabot (`.github/dependabot.yml`) opens
  weekly grouped updates, including for the SHA-pinned actions.

## Simulating League (`crates/mock-league`)
This VM has no League client or game. `mock-league` fakes both on
127.0.0.1: the LCU (HTTPS + WAMP WebSocket, `riot:<token>` auth, rune page
limit, deletable pages, item sets per summoner) and the Live Client Data API
(HTTPS, 404 while no game). Its certificates come from a throwaway root, so
the app's real TLS path (trust one root, tolerate the host name) runs.
Unknown endpoints answer 404 and are recorded as `known: false`.
- In tests: `MockLeague::start(Options { setup, .. })`, drive it with
  `Step`s (`phase`, `champSelect` hover/lock/ARAM, `live` gold/items,
  `noGame`, `dropConnections`, `wait`), read `snapshot()` (rune pages, item
  sets, spells, every request). `tests/end_to_end.rs` shows the wiring.
- The real app: `cargo run -p mock-league -- --scenario
  crates/mock-league/scenarios/ranked-mid-lux.json` (binds 2999 for the
  game, writes `target/mock-league/{lockfile,mock-root.pem,state.json}`),
  then start a **debug** `desktop.exe` with `LOL_OVERLAY_LCU_LOCKFILE` and
  `LOL_OVERLAY_RIOT_PEM` set to the printed paths. Both env vars are ignored
  in release builds. Drive it live with
  `curl -d '{"phase":"InProgress"}' <control>/step`, `GET <control>/state`.
- Scenarios are JSON (`crates/mock-league/scenarios/`); every one is played
  by the `bundled_scenarios_play_through_cleanly` test.
- The mock follows documented/community LCU shapes, not recordings. When
  the user reports real-game behaviour that differs, fix the mock too.

## Trying the overlay without a game
- Whole path (preferred; later overlay parts use the same pipeline):
  `scripts/dev/install-mesa.ps1` once, Vite on 5173, `mock-league.exe
  --control-port 47999`, then `desktop.exe` with the two mock env vars plus
  `GALLIUM_DRIVER=llvmpipe LOL_OVERLAY_ANY_WINDOW=1`. POST steps to
  `/steps` up to `{"phase":"InProgress"}` and a `live` step: the strip
  appears. Ending champ select sends `Clear`, which collapses the column
  even with `LOL_OVERLAY_OPEN`; open it like a player by clicking the
  strip (`scripts/dev/click.ps1 -X -Y`, VM only) and capture it with
  `scripts/dev/screenshot-window.ps1 -Title "LoL Overlay items" -Out x.png`
  (that window's rect only). New `live` steps update it within a second.
- Just the overlay process: `cargo build -p desktop`, then pipe a JSON line into it:
  `(cat view.jsonl; ping -n 15 127.0.0.1 >/dev/null) | LOL_OVERLAY_OPEN=1 LOL_OVERLAY_ANY_WINDOW=1 ./target/debug/desktop.exe --overlay`
  (`OPEN` starts expanded; `ANY_WINDOW` treats any focused window as the
  game). Without `OPEN` it shows the compact strip (needs `gold` set).
  The message must include every `OverlayViewDto` field, or it is ignored.
- The minimap timers: send an `{"type":"objectives","view":{...}}` line
  (see the `ipc-types` test) once a second with a rising `gameTimeMs`;
  `LOL_OVERLAY_OPEN=1` shows the fitting frame. Its window title is
  `LoL Overlay timers`. On Linux the overlay builds with
  `--features eframe/x11` and runs under Xvfb with software GL (needs
  `libxkbcommon-x11-0`); only `input.rs`'s Windows path test fails there.
- Screenshots: find the window titled `LoL Overlay items` with Win32
  `EnumWindows` + `GetWindowRect` from PowerShell and `CopyFromScreen`
  that rect only (don't capture the user's whole screen). The user has 3
  monitors; the primary is at (0,0).
- egui remembers the position in `%APPDATA%\LoL Overlay items\data\app.ron`;
  delete it to test first-run placement. The user may drag the window
  while you test.
- UI demo without Tauri: `cd ui && npm run dev`, then
  `?state=champSelect&page=N`.

## Dev environment (this VM)
- Installed with winget: Rustup (toolchain from `rust-toolchain.toml`),
  Node.js LTS, Visual Studio 2022 Build Tools (C++ workload, needed to link
  on `x86_64-pc-windows-msvc`). `gh` is not installed; push with git.
- Git Bash may not have `~/.cargo/bin` or Node on `PATH` in an old shell:
  `export PATH="$HOME/.cargo/bin:/c/Program Files/nodejs:$PATH"`.
- The VM's display adapter ("Microsoft Basic Display Adapter") has only
  OpenGL 1.1, so the egui overlay fails with "egui_glow requires opengl
  2.0+" until `scripts/dev/install-mesa.ps1` puts Mesa's software OpenGL
  (llvmpipe, pinned SHA-256) next to `target/debug/desktop.exe`. Rerun it
  after `cargo clean`. Start the app with `GALLIUM_DRIVER=llvmpipe`. Dev
  only: the DLLs live in `target/` and are never bundled.
- Debug builds load the UI from `npm run dev` (port 5173), so start Vite
  before `desktop.exe`.
- No Python (the `python3` on PATH is the Microsoft Store stub). Use node
  or PowerShell for scripts. The VM has one CPU: the first full build is slow.
- `apps/desktop/resources/riotgames.pem` is not committed; run
  `scripts/fetch-riot-cert.sh` (checks a pinned SHA-256).

## Environment traps (Windows, Git Bash)
- Don't write `perl -e 's{..}{..}'` edits on Rust code (braces clash) or
  `cmd > tmp && cp tmp file` pipelines (an empty `tmp` wipes the file);
  use the Edit tool.
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
  synthetic fixtures (`fixtures/synthetic/`); `mock-league` is built from
  the same sources, so it proves the wiring, not Riot's exact payloads.

## Open threads (offered, not done)
- Live polling: switch from `allgamedata` (grows with events) to
  `/liveclientdata/activeplayer` + `/liveclientdata/playeritems?riotId=`
  + `/liveclientdata/eventdata?eventID=<next>` (only new events, for the
  objective timers) + `/liveclientdata/gamestats` (the clock); optionally
  poll faster while the column is open.
- Objective timings (`objectives.rs`) come from patch-note summaries for
  26.1; the dragon's 5:00/5:00 was not confirmed for 2026. Check in a game.
- OP.GG region and tier as user settings (now constants in
  `apps/desktop/src/lib.rs` and `crates/opgg/src/lib.rs`).
- `OverlayHost::send` respawns a crashed overlay on every view update with
  no backoff; on a PC where it cannot start (no OpenGL) that is a new
  process about once a second in game. Seen on the VM before Mesa.
- Known limits: typing "p" in chat toggles the column; items sold back
  still count as bought.
- `docs/adr/Changes_todo.md` (runes on hover) is done but left untouched.
