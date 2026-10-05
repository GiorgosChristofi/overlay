# ADR-003: Trust only Riot's root for local HTTPS, ignore host name

Status: accepted, 2026-10-04

## Context

The League client and game serve HTTPS on 127.0.0.1 with certificates
signed by Riot's own root. Their host names do not match `127.0.0.1`, so
normal verification fails. Many tools disable TLS checks entirely.

## Decision

Build a rustls config whose only trust anchor is `riotgames.pem`. Verify the
full chain and signatures; accept a `NotValidForName` error only. Connect
only to `127.0.0.1`. The certificate is fetched at build time
(`scripts/fetch-riot-cert.*`) and bundled as a resource, or pointed to with
`LOL_OVERLAY_RIOT_PEM` in development.

## Consequences

A process impersonating the client without Riot's key is rejected. If Riot
rotates its root, the app needs a new bundle; the error message says so.
