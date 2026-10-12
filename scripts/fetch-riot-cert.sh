#!/usr/bin/env sh
# Downloads Riot's root certificate, used to verify the League client's local HTTPS.
# It is the only trust anchor for the client connection, so the download is
# checked against a pinned hash (Riot's "LoL Game Engineering Certificate
# Authority", valid 2013-2043) before it is kept.
set -eu
expected="da884275737f024b33c93ae5d28bdb002768a3cb73752ab40254a32218193521"
target="$(dirname "$0")/../apps/desktop/resources/riotgames.pem"
tmp="$target.download"
curl -fsSL https://static.developer.riotgames.com/docs/lol/riotgames.pem -o "$tmp"
actual="$(sha256sum "$tmp" | cut -d' ' -f1)"
if [ "$actual" != "$expected" ]; then
  rm -f "$tmp"
  echo "riotgames.pem hash mismatch: got $actual, expected $expected" >&2
  exit 1
fi
mv "$tmp" "$target"
echo "Saved $target"
