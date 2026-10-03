<#
.SYNOPSIS
    One-click post-extraction setup for the cvxx release archive.

.DESCRIPTION
    Removes Windows' "Mark of the Web" (MOTW) from every file extracted
    from cvxx-{version}.zip, including this script itself. MOTW is what
    makes Excel refuse to load cvxx.xlam (it contains a VBA project) with
    "Microsoft has blocked macros from running because the source of this
    file is untrusted" - see INSTALL.md for background.

    Optionally also registers this folder as a Trusted Location in Excel,
    which covers machines where Group Policy still blocks macros from
    internet-sourced files even after unblocking.

    This script never touches cvxx's own add-in registration in Excel
    (File > Options > Add-ins) - that step still needs to be done once by
    hand, per INSTALL.md section 4, because it requires Excel to be closed
    or interacted with directly.

.PARAMETER TrustedLocation
    Also register this folder (and its subfolders) as an Excel Trusted
    Location under HKCU, so macro security policies that block internet
    files no longer apply to it. Safe to re-run; skips if already present.

.EXAMPLE
    .\install.ps1

.EXAMPLE
    .\install.ps1 -TrustedLocation
#>
[CmdletBinding()]
param(
    [switch]$TrustedLocation
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot

function Remove-MarkOfTheWeb {
    param([string]$Path)

    $files = Get-ChildItem -LiteralPath $Path -Recurse -File
    $blocked = @()
    foreach ($file in $files) {
        $stream = Get-Item -LiteralPath $file.FullName -Stream 'Zone.Identifier' -ErrorAction SilentlyContinue
        if ($stream) { $blocked += $file.FullName }
    }

    if ($blocked.Count -eq 0) {
        Write-Host "All $($files.Count) file(s) under $Path are already unblocked." -ForegroundColor Green
        return
    }

    Write-Host "Unblocking $($blocked.Count) of $($files.Count) file(s) under $Path ..."
    Get-ChildItem -LiteralPath $Path -Recurse | Unblock-File

    $stillBlocked = @()
    foreach ($path in $blocked) {
        $stream = Get-Item -LiteralPath $path -Stream 'Zone.Identifier' -ErrorAction SilentlyContinue
        if ($stream) { $stillBlocked += $path }
    }

    if ($stillBlocked.Count -gt 0) {
        Write-Warning "Could not unblock $($stillBlocked.Count) file(s): $($stillBlocked -join ', ')"
        Write-Warning "You may need to run this script from an elevated PowerShell, or unblock these manually (right-click -> Properties -> Unblock)."
    }
    else {
        Write-Host "Unblocked successfully." -ForegroundColor Green
    }
}

function Register-TrustedLocation {
    param([string]$Path)

    # HKCU is not subject to WOW6432Node redirection, so a single path
    # works regardless of whether Office is 32- or 64-bit.
    $officeKey = 'HKCU:\Software\Microsoft\Office'
    if (-not (Test-Path -LiteralPath $officeKey)) {
        Write-Warning "No Office registry key found at $officeKey; skipping Trusted Location setup. Register the folder manually via Excel's Trust Center if needed (see INSTALL.md)."
        return
    }

    # Excel 2016 through current Microsoft 365 builds all use version 16.0
    # for this registry schema.
    $version = '16.0'
    $excelSecurityKey = "$officeKey\$version\Excel\Security"
    $trustedLocationsKey = "$excelSecurityKey\Trusted Locations"
    if (-not (Test-Path -LiteralPath $trustedLocationsKey)) {
        Write-Warning "No Excel $version Trusted Locations key found; skipping. Register the folder manually via Excel's Trust Center if needed (see INSTALL.md)."
        return
    }

    $normalizedPath = (Resolve-Path -LiteralPath $Path).Path.TrimEnd('\') + '\'

    $existing = Get-ChildItem -LiteralPath $trustedLocationsKey -ErrorAction SilentlyContinue |
        Where-Object { $_.PSChildName -like 'Location*' } |
        ForEach-Object {
            [pscustomobject]@{
                Name = $_.PSChildName
                Path = (Get-ItemProperty -LiteralPath $_.PSPath -ErrorAction SilentlyContinue).Path
            }
        }

    $alreadyTrusted = $existing | Where-Object {
        $_.Path -and ($_.Path.TrimEnd('\') + '\') -ieq $normalizedPath
    }
    if ($alreadyTrusted) {
        Write-Host "$normalizedPath is already a Trusted Location ($($alreadyTrusted.Name))." -ForegroundColor Green
        return
    }

    $index = 0
    while ($existing.Name -contains "Location$index") { $index++ }
    $newKeyName = "Location$index"
    $newKeyPath = "$trustedLocationsKey\$newKeyName"

    New-Item -Path $newKeyPath -Force | Out-Null
    New-ItemProperty -Path $newKeyPath -Name 'Path' -Value $normalizedPath -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $newKeyPath -Name 'Description' -Value 'cvxx Excel add-in' -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $newKeyPath -Name 'AllowSubFolders' -Value 1 -PropertyType DWord -Force | Out-Null

    Write-Host "Registered $normalizedPath as Excel Trusted Location '$newKeyName'." -ForegroundColor Green
}

Write-Host "cvxx install helper" -ForegroundColor Cyan
Write-Host "Folder: $root"
Write-Host ''

Remove-MarkOfTheWeb -Path $root

if ($TrustedLocation) {
    Write-Host ''
    Register-TrustedLocation -Path $root
}

Write-Host ''
Write-Host 'Next step: load the add-ins in Excel.' -ForegroundColor Cyan
Write-Host '  File > Options > Add-ins > Manage: Excel Add-ins > Go... > Browse...'
Write-Host "  Select cvxx.xll, then repeat for cvxx.xlam, both in: $root"
Write-Host ''
Write-Host 'See INSTALL.md for details and troubleshooting.'
