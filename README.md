# LoL Overlay

A League of Legends companion for Windows. Rust core, React UI, Tauri 2 shell.

**Phase 1** (this code): connects to the League client, shows your profile and
the session phase, and when you lock in a champion it sets your rune page, item
set and summoner spells. The in-game overlay is Phase 2.

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

No Riot API key is needed for Phase 1: it only uses the local client and Data
Dragon. Register on the Riot Developer Portal before sharing the app with anyone.

### Working on the UI without League

`cd ui && npm run dev`, then open http://localhost:5173. Outside Tauri the UI
uses sample data; pick a scene with `?state=disconnected`, `lobby`,
`champSelect` or `empty`.

## Repository layout

```
crates/domain        pure rules and port traits, no I/O
crates/lcu-client    League client: discovery, TLS, REST, WAMP events
crates/static-data   Data Dragon download + per-patch disk cache
crates/app-core      session service and champ-select import
crates/ipc-types     DTOs shared with React; generates ui/src/bindings
apps/desktop         Tauri shell: window, tray, commands
ui/                  React + TypeScript (Vite)
data/builds          seed builds (schema-versioned JSON)
fixtures/            payloads for contract tests
docs/adr/            architecture decisions
```

The rule: Riot-facing code changes often and lives in adapters; rules change
rarely and live in `domain`. `app-core` only sees port traits, so it is tested
with in-memory fakes.

## Testing

| Command | What it runs |
| --- | --- |
| `cargo test` | 86 Rust tests: unit, property (`proptest`), contract (fixtures), fake-client integration, fake-CDN integration. Also regenerates TS bindings. |
| `cargo test -p static-data --test seed_builds -- --ignored` | Checks seed builds against the live patch (network) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lints, including the Tauri shell. `unwrap`, `expect` and `panic` are denied outside tests |
| `cd ui && npm test` | 12 component tests with a fake backend |
| `cd ui && npm run lint && npm run typecheck` | ESLint (strict TS rules) and `tsc` |

`cargo test` skips `apps/desktop` (see `default-members`) so it runs on any OS.

`crates/lcu-client/tests/support` is a fake League client: REST and the WAMP
socket on one port, as the real client does. It covers subscribing, event
mapping, client restarts, starting mid champ select, and booting clients.

## Adding builds

Append to `data/builds/seed.json`. IDs are checked against the current patch
at runtime: a build with a rune, item or spell that no longer exists is
skipped (or the dead items dropped) and the reason shows in the app.
`championId` is Data Dragon's numeric `key`; `role` is `top`, `jungle`,
`middle`, `bottom`, `support` or `any`.

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
| F-10 | Rune page import on lock-in | Done; replaces only our page unless allowed |
| F-11 | Item set import | Done; keeps the player's own sets |
| F-12 | Summoner spells with Flash key preference | Done |
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
  against the fixture patch but not tuned to the meta.
