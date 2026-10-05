# Downloads Riot's root certificate, used to verify the League client's local HTTPS.
$ErrorActionPreference = "Stop"
$target = Join-Path $PSScriptRoot "..\apps\desktop\resources\riotgames.pem"
Invoke-WebRequest -Uri "https://static.developer.riotgames.com/docs/lol/riotgames.pem" -OutFile $target
if (-not (Select-String -Path $target -Pattern "BEGIN CERTIFICATE" -Quiet)) { throw "Downloaded file is not a PEM certificate" }
Write-Host "Saved $target"
