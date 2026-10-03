<#
.SYNOPSIS
    Checks that every exported CVX.* Excel function is mentioned in the
    an API reference page in the canonical Markdown documentation under
    docs/.

.DESCRIPTION
    This catches a public function that was added without adding it to the
    user-facing reference. It is a coverage check only; reviewers must still
    verify that the documented arguments, return values, errors, and examples
    match the implementation.

.EXAMPLE
    .\scripts\check-public-functions-documented.ps1
#>
[CmdletBinding()]
param(
    [string]$RepositoryRoot = (Join-Path $PSScriptRoot '..')
)

$RepositoryRoot = (Resolve-Path $RepositoryRoot).Path
$excelRoot = Join-Path $RepositoryRoot 'src\excel'
$docsRoot = Join-Path $RepositoryRoot 'docs'

$exportPattern = '#\[export_name\s*=\s*"(?<name>CVX\.[A-Z0-9_]+)"\]'
$functions = @(
    Get-ChildItem -LiteralPath $excelRoot -Filter '*.rs' -File |
        ForEach-Object {
            $source = Get-Content -Raw -LiteralPath $_.FullName
            foreach ($match in [regex]::Matches($source, $exportPattern)) {
                $match.Groups['name'].Value
            }
        } |
        Sort-Object -Unique
)

$markdownFiles = Get-ChildItem -LiteralPath $docsRoot -Filter '*.md' -Recurse -File |
    Where-Object {
        $_.FullName -notmatch '[\\/]html[\\/]' -and
        $_.Name -notin @('README.md', 'SUMMARY.md', 'architecture.md')
    }
$documentation = ($markdownFiles | ForEach-Object {
    Get-Content -Raw -LiteralPath $_.FullName
}) -join "`n"

$missing = @(
    $functions | Where-Object {
        $documentation -notmatch [regex]::Escape($_)
    }
)

if ($missing.Count -gt 0) {
    foreach ($function in $missing) {
        Write-Error "Exported Excel function '$function' is not mentioned in docs/*.md."
    }
    exit 1
}

Write-Host "All $($functions.Count) exported CVX.* functions are mentioned in API reference pages."
