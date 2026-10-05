# Fixtures

Payloads that parser and contract tests run against.

- `synthetic/` — hand-written from Riot's documented shapes and community
  references. They include extra unknown fields on purpose, to prove parsing
  is tolerant. Replace them with recorded payloads once the Phase 0 recorder
  (`apps/dev-tools/recorder`) exists.
- `<patch>/` — recorded, scrubbed payloads from real games (none yet).

Every `*.json` file whose name starts with `champ-select-session` is parsed by
`crates/domain/tests/fixtures.rs`; add new recordings and the test picks them up.
