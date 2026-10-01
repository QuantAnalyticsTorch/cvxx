<#
.SYNOPSIS
    Imports (replacing if present) a VBA module into an .xlam/.xlsm via Excel
    COM automation, without opening the VBE manually.

.DESCRIPTION
    Requires Excel's "Trust access to the VBA project object model" setting
    to be enabled (File > Options > Trust Center > Trust Center Settings >
    Macro Settings). This is a one-time manual toggle; the script does not
    change it since it affects macro security for every Office document on
    the machine, not just this add-in.

.PARAMETER WorkbookPath
    Path to the .xlam/.xlsm to modify. Must not already be open in Excel.

.PARAMETER ModulePath
    Path to the .bas file to import (e.g. xlam/source/modRibbon.bas).

.PARAMETER ModuleName
    Name of the VBA module component; must match the Attribute VB_Name line
    in the .bas file. Defaults to the .bas file's base name.

.EXAMPLE
    .\xlam\tools\Import-VbaModule.ps1 -WorkbookPath .\assets\cvxx.xlam -ModulePath .\xlam\source\modRibbon.bas
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$WorkbookPath,

    [Parameter(Mandatory)]
    [string]$ModulePath,

    [string]$ModuleName
)

$WorkbookPath = (Resolve-Path $WorkbookPath).Path
$ModulePath = (Resolve-Path $ModulePath).Path

if (-not $ModuleName) {
    $ModuleName = [System.IO.Path]::GetFileNameWithoutExtension($ModulePath)
}

$excel = New-Object -ComObject Excel.Application
$excel.Visible = $false
$excel.DisplayAlerts = $false

try {
    $workbook = $excel.Workbooks.Open($WorkbookPath)
    try {
        try {
            $vbProject = $workbook.VBProject
        } catch {
            throw "Could not access VBProject. Enable 'Trust access to the VBA project object model' in Excel's Trust Center (Macro Settings) and try again."
        }

        $components = $vbProject.VBComponents
        if ($null -eq $components) {
            throw "VBComponents is null. Enable 'Trust access to the VBA project object model' in Excel's Trust Center (File > Options > Trust Center > Trust Center Settings > Macro Settings) and try again."
        }

        $existing = $null
        foreach ($component in $components) {
            if ($component.Name -eq $ModuleName) { $existing = $component; break }
        }
        if ($existing) {
            $components.Remove($existing)
        }

        $components.Import($ModulePath) | Out-Null

        $workbook.Save()
    }
    finally {
        $workbook.Close($false)
    }
}
finally {
    $excel.Quit()
    [System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel) | Out-Null
}

Write-Host "Imported $ModuleName from $ModulePath into $WorkbookPath"
