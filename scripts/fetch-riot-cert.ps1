# Downloads Riot's root certificate, used to verify the League client's local HTTPS.
# It is the only trust anchor for the client connection, so the download is
# checked against a pinned hash (Riot's "LoL Game Engineering Certificate
# Authority", valid 2013-2043) before it is kept.
$ErrorActionPreference = "Stop"
$expected = "da884275737f024b33c93ae5d28bdb002768a3cb73752ab40254a32218193521"
$target = Join-Path $PSScriptRoot "..\apps\desktop\resources\riotgames.pem"
$tmp = "$target.download"
Invoke-WebRequest -Uri "https://static.developer.riotgames.com/docs/lol/riotgames.pem" -OutFile $tmp
$actual = (Get-FileHash -Algorithm SHA256 $tmp).Hash.ToLowerInvariant()
if ($actual -ne $expected) {
    Remove-Item $tmp
    throw "riotgames.pem hash mismatch: got $actual, expected $expected"
}
Move-Item -Force $tmp $target
Write-Host "Saved $target"
