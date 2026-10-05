# ADR-002: Generate TypeScript types with ts-rs

Status: accepted, 2026-10-04 (amends the plan, which named specta)

## Context

The UI must not drift from the Rust types it receives. The plan proposed
`specta`/`tauri-specta`, which also generates typed command wrappers but was
still on release candidates when Phase 1 started.

## Decision

Derive `ts_rs::TS` on every DTO in `ipc-types`. `cargo test` writes
`ui/src/bindings/*.ts`; CI fails if they differ from what is committed.
Command names are wrapped by hand in `ui/src/lib/backend.ts` (seven commands).

DTOs use 32-bit integers, because ts-rs maps 64-bit integers to `bigint`,
which JSON never produces.

## Consequences

Types are checked across the boundary; command names are not. Revisit
`tauri-specta` once it is stable, if the command list grows past ~20.
