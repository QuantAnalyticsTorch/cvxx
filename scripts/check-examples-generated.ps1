<#
.SYNOPSIS
    Checks that docs/examples/ contains exactly the eight workbooks
    produced by `gen-examples` and does not contain the retired,
    hand-built Introduction.xlsx.

.DESCRIPTION
    SPEC-0017 replaces the single hand-maintained docs/examples/Introduction.xlsx
    with eight generated workbooks (00-overview.xlsx through
    07-inspecting-results.xlsx). Running `cargo run -p gen-examples` can
    silently produce zero files if it panics before writing anything, or
    leave a stale Introduction.xlsx behind from before this spec; this
    script gives CI a concrete, scriptable check for both failure modes.
    Run this after `cargo run -p gen-examples --release -- --out docs/examples`.

.PARAMETER ExamplesRoot
    Path to the docs/examples/ directory. Defaults to docs/examples next
    to this script.

.EXAMPLE
    cargo run -p gen-examples --release -- --out docs/examples
    .\scripts\check-examples-generated.ps1
#>
[CmdletBinding()]
param(
    [string]$ExamplesRoot = (Join-Path $PSScriptRoot '..\docs\examples')
)

$ExpectedFiles = @(
    '00-overview.xlsx',
    '01-parameters-and-variables.xlsx',
    '02-expressions.xlsx',
    '03-constraints.xlsx',
    '04-problems-and-solving.xlsx',
    '05-quadratic-problems.xlsx',
    '06-vector-matrix-indexing.xlsx',
    '07-inspecting-results.xlsx'
)

if (-not (Test-Path -LiteralPath $ExamplesRoot -PathType Container)) {
    Write-Error "docs/examples was not generated; run 'cargo run -p gen-examples --release -- --out docs/examples' before this check."
    exit 1
}

$ExamplesRoot = (Resolve-Path $ExamplesRoot).Path
$hasError = $false

$actualFiles = Get-ChildItem -LiteralPath $ExamplesRoot -File | Select-Object -ExpandProperty Name

foreach ($expected in $ExpectedFiles) {
    if ($actualFiles -notcontains $expected) {
        Write-Error "docs/examples is missing expected workbook '$expected'."
        $hasError = $true
    }
}

foreach ($actual in $actualFiles) {
    if ($ExpectedFiles -notcontains $actual) {
        Write-Error "docs/examples contains unexpected file '$actual' (expected exactly the eight generated workbooks)."
        $hasError = $true
    }
}

if ($actualFiles -contains 'Introduction.xlsx') {
    Write-Error "docs/examples/Introduction.xlsx is still present; it was superseded by 00-overview.xlsx and must be deleted."
    $hasError = $true
}

foreach ($expected in $ExpectedFiles) {
    $path = Join-Path $ExamplesRoot $expected
    if ((Test-Path -LiteralPath $path) -and (Get-Item -LiteralPath $path).Length -eq 0) {
        Write-Error "docs/examples/$expected was generated but is empty."
        $hasError = $true
    }
}

if ($hasError) {
    Write-Error "docs/examples/ does not match the expected set of generated example workbooks."
    exit 1
}

Write-Host "docs/examples/ contains exactly the eight expected generated workbooks."
