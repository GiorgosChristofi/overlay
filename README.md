# LoL Overlay

A League of Legends companion for Windows. Rust core, React UI, Tauri 2 shell.

**Phase 1** (this code): connects to the League client, shows your profile and
the session phase, and as soon as you hover or lock in a champion it sets your
rune page, item set and summoner spells from OP.GG statistics for your role (or ARAM). The
app lists OP.GG's top rune pages with win and pick rates and draws each as a
rune tree; hover a rune to see what it does, click a page to switch to it.

**Phase 2** (started): in game, press **P** and a movable column shows what
to build: the next item broken into its components with your gold, and the
rest of the build, with bought items removed.

LoL Overlay is not endorsed by Riot Games and does not reflect the views or
opinions of Riot Games or anyone officially involved in producing or managing
Riot Games properties. Riot Games and all associated properties are trademarks
or registered trademarks of Riot Games, Inc.

## Getting started

Prerequisites: Windows 10/11 (WebView2 is preinstalled), [rustup](https://rustup.rs)
(the toolchain in `rust-toolchain.toml` installs itself), Node.js 22, and the
Tauri CLI: `cargo install tauri-cli --version "^2" --locked`.

```powershell
./scripts/fetch-riot-cert.ps1        # Riot's root certificate for the client's local HTTPS
cd ui; npm ci; cd ..
cd apps/desktop; cargo tauri dev     # starts Vite and the app
```

Start League of Legends; the app connects within a couple of seconds.
Release installer: `cargo tauri build` in `apps/desktop`.

No Riot API key is needed for Phase 1: it only uses the local client, Data
Dragon and OP.GG. Register on the Riot Developer Portal before sharing the app with anyone.

### Working on the UI without League

`cd ui && npm run dev`, then open http://localhost:5173. Outside Tauri the UI
uses sample data; pick a scene with `?state=disconnected`, `lobby`,
`champSelect` or `empty`.

## Repository layout

```
crates/domain        pure rules and port traits, no I/O
crates/lcu-client    League client: discovery, TLS, REST, WAMP events
crates/static-data   Data Dragon download + per-patch disk cache
crates/opgg          OP.GG champion statistics -> builds (runes, spells, items)
crates/app-core      session service and champ-select import
crates/ipc-types     DTOs shared with React; generates ui/src/bindings
crates/mock-league   fake League client + game for end-to-end tests and manual runs
apps/desktop         Tauri shell: window, tray, commands
apps/overlay         in-game item column (egui), run as `desktop --overlay`
ui/                  React + TypeScript (Vite)
data/builds          seed builds, used when OP.GG is unreachable
fixtures/            payloads for contract tests
docs/adr/            architecture decisions
```

The rule: Riot-facing code changes often and lives in adapters; rules change
rarely and live in `domain`. `app-core` only sees port traits, so it is tested
with in-memory fakes.

## Testing

| Command | What it runs |
| --- | --- |
| `cargo test` | 163 Rust tests: unit, property (`proptest`), contract (fixtures), fake-client integration, fake-CDN integration. Also regenerates TS bindings. |
| `cargo test -p static-data --test seed_builds -- --ignored` | Checks seed builds against the live patch (network) |
| `cargo test -p opgg -- --ignored` | Checks the live OP.GG API still parses (network) |
| `cargo test -p overlay` | The in-game column's state and input rules (Windows, like the shell) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lints, including the Tauri shell. `unwrap`, `expect` and `panic` are denied outside tests |
| `cd ui && npm test` | 22 component tests with a fake backend |
| `cd ui && npm run lint && npm run typecheck` | ESLint (strict TS rules) and `tsc` |

`cargo test` skips `apps/desktop` (see `default-members`) so it runs on any OS.

`crates/lcu-client/tests/support` is a fake League client: REST and the WAMP
socket on one port, as the real client does. It covers subscribing, event
mapping, client restarts, starting mid champ select, and booting clients.

### Playing a game without League

`crates/mock-league` fakes the League client (HTTPS, WAMP events, password,
rune page limit, item sets) and the game's Live Client Data API, with
certificates from a throwaway root. `cargo test -p mock-league` runs the app's
core, connector and live reader against it: a whole game, an impostor client,
full rune pages, client restarts, ARAM, and a check that the app only calls
the endpoints it needs.

To watch the real app play a scripted game:

```
cargo run -p mock-league -- --scenario crates/mock-league/scenarios/ranked-mid-lux.json
# in another terminal, with the two paths it prints:
LOL_OVERLAY_LCU_LOCKFILE=... LOL_OVERLAY_RIOT_PEM=... cargo run -p desktop
```

The overrides only work in debug builds. While it runs, `curl -d
'{"phase":"InProgress"}' <control url>/step` applies a step and
`<control url>/state` shows what the app did.

## Where builds come from

When you hover or lock in a champion the app asks OP.GG (`lol-api-champion.op.gg`, the JSON API behind
the website) for the champion's statistics: all regions combined
(`OPGG_REGION` in `apps/desktop/src/lib.rs`; a code like `EUNE` limits it to
one server), Emerald+, in the assigned role. Without an
assigned role it uses the champion's most played role; in ARAM (a champ
select with a bench) it uses ARAM statistics. From the response it takes:

- **Rune pages:** the most common page of each of the top five keystone and
  secondary-tree pairs. The first is applied; the others can be chosen in
  the app. Pages that are invalid this patch are left out.
- **Summoner spells:** the most picked pair, with Flash on your key.
- **Item set:** starting items, core build, boots and six situational items.

Answers are cached for 30 minutes. This API is undocumented and can change
without notice; the parser in `crates/opgg/src/parse.rs` tolerates missing
fields, and if OP.GG fails the seed builds are used and the app says so.

### Adding seed builds

Append to `data/builds/seed.json`. IDs are checked against the current patch
at runtime: a build with a rune, item or spell that no longer exists is
skipped (or the dead items dropped) and the reason shows in the app.
`championId` is Data Dragon's numeric `key`; `role` is `top`, `jungle`,
`middle`, `bottom`, `support` or `any`.

## In-game item column (Phase 2)

Press **P** in game (the shop key) to open the full column with the shop;
**P** again, **Esc** or **–** shrinks it to a one-line strip with the next
item and the gold it still needs. Click the strip to open the column without
the shop. Drag the column by its header; it remembers where you put it. It
only shows while the game window is focused. The column shows:

- **What to buy now**: at the start of the game, the starting items. After
  that, the boots (until bought, since many players buy them later) and the
  next item: the first core item not bought yet, then situational. Each is
  broken into its recipe; components you hold are crossed out, and gold
  amounts turn green when your current gold covers them.
- **Core, boots, situational**: the rest of the build, with how often each
  situational item is built. Items leave the list once bought.
- **Your choices**: click an item to build it next instead (click again, or
  "automatic", to undo); click its **×** to take it off the list ("Restore"
  at the bottom brings removed items back). Choices last for the game.

How it works: the desktop app starts a second process of itself
(`desktop --overlay`, egui) and sends it the view as JSON lines. It reads
gold and items once a second from Riot's Live Client Data API
(`https://127.0.0.1:2999`, local and read-only) and recipes from Data
Dragon. The overlay only reads key state and which window is focused; it
never touches game memory. It reacts only while League's game window is
focused. Turn it off in Settings → In game.

Requirements and limits:

- League must run in **Borderless** or **Windowed** mode; nothing can draw
  over exclusive fullscreen.
- The game does not tell anyone whether its shop is open (the Live Client
  Data API has no UI state), so the column follows your P presses. If you
  close the shop with its own X, shrink the column with its **–** rather than
  pressing P again.
- Typing "p" in chat also toggles it, and items sold back still count as bought.
- To work on it without a game: pipe an `OverlayMessage` JSON line into
  `desktop --overlay` with `LOL_OVERLAY_OPEN=1` (start open) and
  `LOL_OVERLAY_ANY_WINDOW=1` (treat any window as the game). Clicks come
  out on stdout as `OverlayCommand` JSON lines.

## Phase 1 status

| ID | Requirement | State |
| --- | --- | --- |
| F-01 | Detect the client starting and stopping | Done; reconnect covered by tests |
| F-02 | Track the gameflow phase | Done |
| F-03 | Show Riot ID, icon, level | Done |
| F-04 | Static data per patch with cache | Done; offline fallback |
| F-05 | Settings persisted locally | Done; atomic writes, corrupt file kept aside |
| F-06 | Tray, start with Windows, start minimised | Done; single instance |
| F-07 | Legal notice / About | Done |
| F-10 | Rune page import on lock-in | Done; OP.GG pages per role, choosable in the app |
| F-11 | Item set import | Done; from OP.GG, keeps the player's own sets |
| F-12 | Summoner spells with Flash key preference | Done; from OP.GG |
| N-01 | Rotating log file, open logs folder | Done |

### Not yet verified

- **Nothing has run against a real League client yet.** LCU endpoint paths
  and payload shapes come from community documentation and are covered by
  synthetic fixtures. Gate 1 (import under 1 s across 20 real games) is open.
  First real run: check the log file, then record real payloads into
  `fixtures/<patch>/` with the Phase 0 recorder (still to build).
- The Tauri shell compiles and passes clippy on Linux; the Windows installer
  build runs in CI and has not been run by hand.
- Seed builds are three examples (Garen top, Lux mid, Jinx bot), valid
  against the fixture patch but not tuned to the meta. They are only the
  offline fallback now.
