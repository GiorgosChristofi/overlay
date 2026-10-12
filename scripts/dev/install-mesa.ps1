# Development only: a software OpenGL driver (Mesa llvmpipe) for machines
# without OpenGL 2.0, such as VMs on "Microsoft Basic Display Adapter".
# The in-game column (egui on OpenGL) cannot open there otherwise.
#
# Copies opengl32.dll + libgallium_wgl.dll next to target\<profile>\desktop.exe;
# Windows loads an opengl32.dll from the exe's folder before System32. Never
# ship these: the installer bundles only what tauri.conf.json lists. Run the
# app with GALLIUM_DRIVER=llvmpipe.
#
# Downloads are pinned by SHA-256 (from GitHub's release asset digests):
# Mesa from pal1000/mesa-dist-win, and 7-Zip's standalone 7zr.exe from
# ip7z/7zip to unpack it (Windows' tar cannot read 7z).
param([string]$BuildProfile = "debug")
$ErrorActionPreference = "Stop"

$mesaVersion = "26.2.4"
$mesaUrl = "https://github.com/pal1000/mesa-dist-win/releases/download/$mesaVersion/mesa3d-$mesaVersion-release-msvc.7z"
$mesaSha = "351fc8c8b695878ffb3eaa044b3ead08672a48b1a045e3c3e3975811df0f6695"
$sevenZipUrl = "https://github.com/ip7z/7zip/releases/download/26.04/7zr.exe"
$sevenZipSha = "256feca8e274e5da655e2a284fabafd9f554365eb164862089dacd4e8276d282"

$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$tools = Join-Path $root "target\dev-tools\mesa-$mesaVersion"
$dest = Join-Path $root "target\$BuildProfile"
New-Item -ItemType Directory -Force $tools, $dest | Out-Null

function Get-Pinned([string]$Url, [string]$Sha, [string]$Path) {
    if (-not (Test-Path $Path) -or (Get-FileHash -Algorithm SHA256 $Path).Hash.ToLowerInvariant() -ne $Sha) {
        Invoke-WebRequest -Uri $Url -OutFile $Path
    }
    $actual = (Get-FileHash -Algorithm SHA256 $Path).Hash.ToLowerInvariant()
    if ($actual -ne $Sha) {
        Remove-Item $Path
        throw "$Url hash mismatch: got $actual, expected $Sha"
    }
}

$archive = Join-Path $tools "mesa.7z"
$sevenZip = Join-Path $tools "7zr.exe"
Get-Pinned $sevenZipUrl $sevenZipSha $sevenZip
Get-Pinned $mesaUrl $mesaSha $archive

& $sevenZip e $archive "-o$tools" "x64/opengl32.dll" "x64/libgallium_wgl.dll" -y | Out-Null
if ($LASTEXITCODE -ne 0) { throw "7zr failed with exit code $LASTEXITCODE" }
Copy-Item (Join-Path $tools "opengl32.dll"), (Join-Path $tools "libgallium_wgl.dll") $dest -Force
Write-Host "Mesa $mesaVersion (llvmpipe) installed in $dest"
Write-Host "Run the app with GALLIUM_DRIVER=llvmpipe."
