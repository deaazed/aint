# Installs the latest aint release for Windows from GitHub Releases -
# no cargo, no cloning the repo. See https://github.com/deaazed/aint
# for the source.
#
# Usage: irm https://raw.githubusercontent.com/deaazed/aint/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$repo = "deaazed/aint"
$installDir = if ($env:AINT_INSTALL_DIR) { $env:AINT_INSTALL_DIR } else { "$env:USERPROFILE\.aint" }
$binDir = Join-Path $installDir "bin"

$asset = "aint-windows-x86_64"
$url = "https://github.com/$repo/releases/latest/download/$asset.zip"

Write-Output "downloading $asset.zip..."
$tmp = New-Item -ItemType Directory -Path (Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid()))
$zipPath = Join-Path $tmp "$asset.zip"

try {
    Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing
} catch {
    Write-Error "could not download $url - see https://github.com/$repo/releases for what's actually published"
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    exit 1
}

New-Item -ItemType Directory -Force -Path $binDir | Out-Null
Expand-Archive -Path $zipPath -DestinationPath $tmp -Force
Move-Item -Force (Join-Path $tmp "aint.exe") (Join-Path $binDir "aint.exe")
Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue

Write-Output "installed aint to $binDir\aint.exe"

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$binDir*") {
    Write-Output ""
    Write-Output "$binDir isn't on your PATH yet - add it permanently with:"
    Write-Output "  [Environment]::SetEnvironmentVariable('Path', `"`$env:Path;$binDir`", 'User')"
    Write-Output "or just for this session:"
    Write-Output "  `$env:Path += `";$binDir`""
}

Write-Output ""
Write-Output "verify with: aint --version"
