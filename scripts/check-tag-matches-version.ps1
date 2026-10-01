<#
.SYNOPSIS
    Checks that an annotated release tag (e.g. `v0.5.0-beta.1`) matches
    Cargo.toml's package version exactly.

.DESCRIPTION
    Per SPEC-0001, tags use the `v{major}.{minor}.{patch}[-prerelease]`
    format and must match `package.version` in Cargo.toml exactly. Unlike
    check-version-sync.ps1, this check fails the build (it gates
    publishing a release for the wrong version).

.PARAMETER Tag
    The git tag being released, e.g. `v1.2.3` or `v0.5.0-beta.1`.

.PARAMETER RepoRoot
    Path to the repository root. Defaults to the parent of this script.

.EXAMPLE
    .\scripts\check-tag-matches-version.ps1 -Tag v1.0.0
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Tag,

    [string]$RepoRoot = (Join-Path $PSScriptRoot '..')
)

$RepoRoot = (Resolve-Path $RepoRoot).Path

if ($Tag -notmatch '^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$') {
    Write-Error "Tag '$Tag' does not match the required 'vMAJOR.MINOR.PATCH[-prerelease]' format."
    exit 1
}
$tagVersion = $Matches[1]

$cargoToml = Get-Content -Raw -LiteralPath (Join-Path $RepoRoot 'Cargo.toml')
if ($cargoToml -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
    Write-Error "Could not find package.version in Cargo.toml"
    exit 1
}
$cargoVersion = $Matches[1]

if ($tagVersion -ne $cargoVersion) {
    Write-Error "Git tag '$Tag' (version '$tagVersion') does not match Cargo.toml package.version ('$cargoVersion')."
    exit 1
}

Write-Host "Tag $Tag matches Cargo.toml version $cargoVersion."
