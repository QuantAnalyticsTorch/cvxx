<#
.SYNOPSIS
    Checks that the committed version.txt matches Cargo.toml's package
    version.

.DESCRIPTION
    `build.rs` regenerates version.txt from `CARGO_PKG_VERSION` on every
    `cargo build`, so this only catches a version.txt that was committed out
    of sync with a Cargo.toml version bump (e.g. Cargo.toml was edited by
    hand without re-running `cargo build` and committing the result).
    Must run before any cargo command in CI, since cargo would otherwise
    silently regenerate version.txt first. See SPEC-0001's Error Handling:
    this check warns but never fails the build.

.PARAMETER RepoRoot
    Path to the repository root. Defaults to the parent of this script.

.EXAMPLE
    .\scripts\check-version-sync.ps1
#>
[CmdletBinding()]
param(
    [string]$RepoRoot = (Join-Path $PSScriptRoot '..')
)

$RepoRoot = (Resolve-Path $RepoRoot).Path

$cargoToml = Get-Content -Raw -LiteralPath (Join-Path $RepoRoot 'Cargo.toml')
if ($cargoToml -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
    Write-Error "Could not find package.version in Cargo.toml"
    exit 1
}
$cargoVersion = $Matches[1]

$versionFile = Join-Path $RepoRoot 'version.txt'
if (-not (Test-Path -LiteralPath $versionFile)) {
    Write-Host "::warning::version.txt is missing (Cargo.toml version is $cargoVersion). Run 'cargo build' and commit the generated version.txt."
    exit 0
}

$fileVersion = (Get-Content -Raw -LiteralPath $versionFile).Trim()
if ($fileVersion -ne $cargoVersion) {
    Write-Host "::warning::version.txt ('$fileVersion') is out of sync with Cargo.toml ('$cargoVersion'). Run 'cargo build' and commit the regenerated version.txt."
    exit 0
}

Write-Host "version.txt ($fileVersion) matches Cargo.toml."
