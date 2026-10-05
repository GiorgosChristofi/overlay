#!/usr/bin/env sh
# Downloads Riot's root certificate, used to verify the League client's local HTTPS.
set -eu
target="$(dirname "$0")/../apps/desktop/resources/riotgames.pem"
curl -fsSL https://static.developer.riotgames.com/docs/lol/riotgames.pem -o "$target"
grep -q "BEGIN CERTIFICATE" "$target" || { echo "Downloaded file is not a PEM certificate" >&2; exit 1; }
echo "Saved $target"
