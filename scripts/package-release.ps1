<#
.SYNOPSIS
    Assembles the cvxx-{version}.zip release archive described in
    SPEC-0001 (release pipeline and versioning).

.DESCRIPTION
    Collects the locally built 64-bit XLL, the committed XLAM, the rendered
    offline docs, the example workbooks, INSTALL.md, and a SHA256SUMS.txt
    checksum file into dist\cvxx-{version}.zip. Run this on the maintainer's
    machine before uploading the archive to the draft GitHub Release created
    by .github/workflows/release.yml; CI does not build or attach the
    archive itself (no XLAM build in CI, no code signing).

    Fails loudly (rather than silently skipping) when a required input is
    missing or empty, per SPEC-0001's Error Handling section.

.PARAMETER SkipBuild
    Skip running `cargo build --release` and `mdbook build docs` before
    packaging; use this if you already built both and just want to
    re-assemble the archive.

.EXAMPLE
    .\scripts\package-release.ps1
#>
[CmdletBinding()]
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

function Get-CargoVersion {
    $cargoToml = Get-Content -Raw -LiteralPath (Join-Path $repoRoot 'Cargo.toml')
    if ($cargoToml -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
        throw "Could not find package.version in Cargo.toml"
    }
    return $Matches[1]
}

function Assert-NonEmptyDirectory {
    param([string]$Path, [string]$Description)
    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        throw "$Description is missing: $Path"
    }
    if (-not (Get-ChildItem -LiteralPath $Path -Recurse -File | Select-Object -First 1)) {
        throw "$Description is empty: $Path"
    }
}

$version = Get-CargoVersion
Write-Host "Packaging cvxx $version"

if (-not $SkipBuild) {
    Push-Location $repoRoot
    try {
        cargo build --release
        if ($LASTEXITCODE -ne 0) { throw "cargo build --release failed" }
        mdbook build docs
        if ($LASTEXITCODE -ne 0) { throw "mdbook build failed" }
        cargo run -p gen-examples --release -- --out docs/examples
        if ($LASTEXITCODE -ne 0) { throw "cargo run -p gen-examples failed" }
    }
    finally {
        Pop-Location
    }
}

$xll = Join-Path $repoRoot 'target\release\cvxx.xll'
$dll = Join-Path $repoRoot 'target\release\cvxx.dll'
$xlam = Join-Path $repoRoot 'assets\cvxx.xlam'
$docsHtml = Join-Path $repoRoot 'docs\html'
$docsExamples = Join-Path $repoRoot 'docs\examples'
$installMd = Join-Path $repoRoot 'INSTALL.md'

# `cargo build --release` produces cvxx.dll (the `cdylib` crate-type), never
# cvxx.xll directly; an XLL is simply a DLL with the exports xladd/Excel
# expect, renamed with the `.xll` extension. Keep this copy in sync with the
# freshly built DLL rather than trusting a stale .xll left over from an
# earlier build.
if (Test-Path -LiteralPath $dll -PathType Leaf) {
    Copy-Item -LiteralPath $dll -Destination $xll -Force
}

if (-not (Test-Path -LiteralPath $xll -PathType Leaf)) {
    throw "cvxx.xll not found at $xll; run 'cargo build --release' first."
}
if (-not (Test-Path -LiteralPath $xlam -PathType Leaf)) {
    throw "cvxx.xlam not found at $xlam; build it per .github/skills/cvxx-excel-ribbon-xlam/SKILL.md."
}
Assert-NonEmptyDirectory -Path $docsHtml -Description "docs/html (run 'mdbook build docs')"
Assert-NonEmptyDirectory -Path $docsExamples -Description 'docs/examples'
if (-not (Test-Path -LiteralPath $installMd -PathType Leaf)) {
    throw "INSTALL.md not found at $installMd"
}

$stagingRoot = Join-Path $repoRoot "dist\cvxx-$version"
if (Test-Path -LiteralPath $stagingRoot) {
    Remove-Item -LiteralPath $stagingRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $stagingRoot | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stagingRoot 'docs') | Out-Null

Copy-Item -LiteralPath $xll -Destination (Join-Path $stagingRoot 'cvxx.xll')
Copy-Item -LiteralPath $xlam -Destination (Join-Path $stagingRoot 'cvxx.xlam')
Copy-Item -LiteralPath $installMd -Destination (Join-Path $stagingRoot 'INSTALL.md')
Copy-Item -LiteralPath $docsHtml -Destination (Join-Path $stagingRoot 'docs\html') -Recurse
Copy-Item -LiteralPath $docsExamples -Destination (Join-Path $stagingRoot 'docs\examples') -Recurse

# Checksums cover every packaged file except the checksum file itself, with
# paths relative to the archive root using forward slashes so the listing
# reads the same way regardless of platform.
$sumsPath = Join-Path $stagingRoot 'SHA256SUMS.txt'
$lines = Get-ChildItem -LiteralPath $stagingRoot -Recurse -File |
    ForEach-Object {
        $relative = $_.FullName.Substring($stagingRoot.Length + 1) -replace '\\', '/'
        $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $relative"
    }
Set-Content -LiteralPath $sumsPath -Value $lines -Encoding ascii

$distDir = Join-Path $repoRoot 'dist'
$zipPath = Join-Path $distDir "cvxx-$version.zip"
if (Test-Path -LiteralPath $zipPath) {
    Remove-Item -LiteralPath $zipPath -Force
}
Compress-Archive -Path (Join-Path $stagingRoot '*') -DestinationPath $zipPath

Write-Host "Wrote $zipPath"
