# ADR-001: Tauri 2 shell, Rust workspace core, React UI

Status: accepted, 2026-10-04

## Context

The overlay needs a native process that talks to the League client, a UI,
and (from Phase 2) a transparent window over the game. The owner wants Rust
for all logic and React for the UI.

## Decision

- One Tauri 2 app. Rust owns every network call, file and timer; React only
  renders view models it receives.
- A Cargo workspace split by rate of change: `domain` (pure rules, no I/O),
  adapter crates (`lcu-client`, `static-data`), `app-core` (services wired
  through port traits), `ipc-types` (DTOs), and a thin `apps/desktop` shell.
- `default-members` excludes `apps/desktop`, so `cargo test` runs on any OS
  without WebView system libraries; Windows CI builds the shell.

## Consequences

- Riot changes land in adapters and fixtures, not in rules or UI.
- Every service is tested against fakes; no League install needed in CI.
- Tauri releases set the minimum Rust version (currently 1.90).
