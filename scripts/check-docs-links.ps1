<#
.SYNOPSIS
    Checks docs/*.md for relative Markdown links that point at a missing
    file, and confirms every page listed in docs/SUMMARY.md rendered to
    docs/html/*.html.

.DESCRIPTION
    `mdbook build` does not itself fail on a broken relative link, so this
    script gives SPEC-0001's "CI must fail if the mdbook build reports
    broken internal links or fails to render any page under docs/"
    requirement a concrete, scriptable check. Run this after
    `mdbook build docs`.

.PARAMETER DocsRoot
    Path to the docs/ directory. Defaults to docs/ next to this script.

.EXAMPLE
    mdbook build docs
    .\scripts\check-docs-links.ps1
#>
[CmdletBinding()]
param(
    [string]$DocsRoot = (Join-Path $PSScriptRoot '..\docs')
)

$DocsRoot = (Resolve-Path $DocsRoot).Path
$hasError = $false
$linkPattern = '\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)'

# 1. Every relative link in docs/*.md (excluding the rendered docs/html
#    output) must resolve to an existing file.
$mdFiles = Get-ChildItem -LiteralPath $DocsRoot -Filter '*.md' -Recurse -File |
    Where-Object { $_.FullName -notmatch '[\\/]html[\\/]' }

foreach ($file in $mdFiles) {
    $content = Get-Content -Raw -LiteralPath $file.FullName
    foreach ($match in [regex]::Matches($content, $linkPattern)) {
        $target = $match.Groups[1].Value
        if ($target -match '^(https?:|mailto:|#)') { continue }

        $targetPath = $target.Split('#')[0]
        if ([string]::IsNullOrWhiteSpace($targetPath)) { continue }

        $resolved = Join-Path $file.DirectoryName $targetPath
        if (-not (Test-Path -LiteralPath $resolved)) {
            Write-Host "::error file=$($file.FullName)::Broken link to '$target' (resolved: $resolved)"
            $hasError = $true
        }
    }
}

# 2. Every page listed in SUMMARY.md must have rendered to a non-empty
#    docs/html/*.html file.
$htmlRoot = Join-Path $DocsRoot 'html'
if (-not (Test-Path -LiteralPath $htmlRoot -PathType Container)) {
    Write-Error "docs/html was not generated; run 'mdbook build docs' before this check."
    exit 1
}

$summaryContent = Get-Content -Raw -LiteralPath (Join-Path $DocsRoot 'SUMMARY.md')
$isFirstPage = $true
foreach ($match in [regex]::Matches($summaryContent, $linkPattern)) {
    $target = $match.Groups[1].Value
    if ($target -match '^(https?:|mailto:|#)') { continue }
    $targetPath = $target.Split('#')[0]
    if (-not $targetPath.EndsWith('.md')) { continue }

    # mdbook renders SUMMARY.md's first linked page (the book's index
    # chapter, conventionally README.md) to index.html, not <name>.html.
    if ($isFirstPage) {
        $htmlName = 'index.html'
        $isFirstPage = $false
    } else {
        $htmlName = [System.IO.Path]::ChangeExtension($targetPath, '.html')
    }
    $htmlPath = Join-Path $htmlRoot $htmlName
    $rendered = Test-Path -LiteralPath $htmlPath
    if ($rendered -and (Get-Item -LiteralPath $htmlPath).Length -eq 0) {
        $rendered = $false
    }
    if (-not $rendered) {
        Write-Error "'$targetPath' from SUMMARY.md did not render to a non-empty $htmlPath"
        $hasError = $true
    }
}

if ($hasError) {
    Write-Error "Broken internal documentation links or missing rendered pages were found."
    exit 1
}

Write-Host "All relative documentation links resolve and every SUMMARY.md page rendered."
