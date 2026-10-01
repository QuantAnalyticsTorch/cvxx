<#
.SYNOPSIS
    Injects or replaces the Office Fluent Ribbon customUI14 part inside an
    Excel OPC package (.xlam/.xlsm) without using a GUI editor.

.DESCRIPTION
    .xlam/.xlsm files are ZIP archives. This adds/updates four entries:
      - customUI/customUI14.xml  the ribbon XML itself (2010+ schema)
      - [Content_Types].xml      content type override for that part
      - _rels/.rels              relationship pointing at the part
      - docProps/core.xml        the dc:title document property

    Excel's Add-ins manager displays an xlam's dc:title document property
    (falling back to the file name if blank) as its add-in name. The XLL
    (cvxx.xll) is registered separately in native code and also shows up as
    "cvxx" there by default, so this script sets the xlam's title to a
    distinct value to tell the two apart in the Add-ins list, without any
    change to the XLL.

    Run this against an .xlam that was already saved once from Excel (so the
    base package structure exists) and that already has modRibbon imported,
    since this script only touches the ribbon XML and document title, not VBA.

.PARAMETER WorkbookPath
    Path to the .xlam/.xlsm file to modify in place. Close it in Excel first.

.PARAMETER CustomUiXmlPath
    Path to the customUI XML source to inject (e.g. xlam/source/customUI.xml).

.PARAMETER AddInTitle
    Document title shown for this add-in in Excel's Add-ins manager. Defaults
    to a name distinct from the XLL's "cvxx" so the two don't look identical.

.EXAMPLE
    .\xlam\tools\Inject-CustomUI.ps1 -WorkbookPath .\assets\cvxx.xlam -CustomUiXmlPath .\xlam\source\customUI.xml
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$WorkbookPath,

    [Parameter(Mandatory)]
    [string]$CustomUiXmlPath,

    [string]$AddInTitle = 'cvxx Ribbon & Docs'
)

Add-Type -AssemblyName System.IO.Compression.FileSystem

$WorkbookPath = (Resolve-Path $WorkbookPath).Path
$CustomUiXmlPath = (Resolve-Path $CustomUiXmlPath).Path

$partName = 'customUI/customUI14.xml'
$relType = 'http://schemas.microsoft.com/office/2007/relationships/ui/extensibility'
$contentType = 'application/xml'

function Set-ZipEntryText {
    param($Zip, [string]$EntryName, [string]$Text)

    $existing = $Zip.GetEntry($EntryName)
    if ($existing) { $existing.Delete() }

    $entry = $Zip.CreateEntry($EntryName)
    $stream = $entry.Open()
    try {
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($Text)
        $stream.Write($bytes, 0, $bytes.Length)
    } finally {
        $stream.Close()
    }
}

function Get-ZipEntryXml {
    param($Zip, [string]$EntryName)

    $entry = $Zip.GetEntry($EntryName)
    $reader = New-Object System.IO.StreamReader($entry.Open())
    try {
        $doc = New-Object System.Xml.XmlDocument
        $doc.LoadXml($reader.ReadToEnd())
        return $doc
    } finally {
        $reader.Close()
    }
}

function Set-CorePropertyTitle {
    param($Zip, [string]$Title)

    $entryName = 'docProps/core.xml'
    if (-not $Zip.GetEntry($entryName)) {
        Write-Warning "$entryName not found in package; skipping add-in title update."
        return
    }

    $doc = Get-ZipEntryXml -Zip $Zip -EntryName $entryName
    $dcNs = 'http://purl.org/dc/elements/1.1/'

    $titleNode = $doc.DocumentElement.SelectSingleNode("*[local-name()='title' and namespace-uri()='$dcNs']")
    if (-not $titleNode) {
        $titleNode = $doc.CreateElement('dc', 'title', $dcNs)
        $doc.DocumentElement.AppendChild($titleNode) | Out-Null
    }
    $titleNode.InnerText = $Title

    Set-ZipEntryText -Zip $Zip -EntryName $entryName -Text $doc.OuterXml
}

$zip = [System.IO.Compression.ZipFile]::Open($WorkbookPath, 'Update')
try {
    # 1. Ribbon XML part.
    $customUiText = [System.IO.File]::ReadAllText($CustomUiXmlPath)
    Set-ZipEntryText -Zip $zip -EntryName $partName -Text $customUiText

    # 2. [Content_Types].xml override.
    $ctDoc = Get-ZipEntryXml -Zip $zip -EntryName '[Content_Types].xml'
    $ctNs = $ctDoc.DocumentElement.NamespaceURI
    $overridePartName = '/' + $partName
    $existingOverride = $ctDoc.SelectSingleNode("//*[local-name()='Override' and @PartName='$overridePartName']")
    if (-not $existingOverride) {
        $override = $ctDoc.CreateElement('Override', $ctNs)
        $override.SetAttribute('PartName', $overridePartName)
        $override.SetAttribute('ContentType', $contentType)
        $ctDoc.DocumentElement.AppendChild($override) | Out-Null
    }
    Set-ZipEntryText -Zip $zip -EntryName '[Content_Types].xml' -Text $ctDoc.OuterXml

    # 3. _rels/.rels relationship.
    $relsDoc = Get-ZipEntryXml -Zip $zip -EntryName '_rels/.rels'
    $relsNs = $relsDoc.DocumentElement.NamespaceURI
    $existingRel = $relsDoc.SelectSingleNode("//*[local-name()='Relationship' and @Type='$relType']")
    if ($existingRel) {
        $existingRel.SetAttribute('Target', $partName)
    } else {
        $maxId = 0
        foreach ($idAttr in $relsDoc.SelectNodes("//*[local-name()='Relationship']/@Id")) {
            if ($idAttr.Value -match '^rId(\d+)$') {
                $n = [int]$Matches[1]
                if ($n -gt $maxId) { $maxId = $n }
            }
        }
        $rel = $relsDoc.CreateElement('Relationship', $relsNs)
        $rel.SetAttribute('Id', "rId$($maxId + 1)")
        $rel.SetAttribute('Type', $relType)
        $rel.SetAttribute('Target', $partName)
        $relsDoc.DocumentElement.AppendChild($rel) | Out-Null
    }
    Set-ZipEntryText -Zip $zip -EntryName '_rels/.rels' -Text $relsDoc.OuterXml

    # 4. docProps/core.xml dc:title (the name Excel's Add-ins manager shows).
    Set-CorePropertyTitle -Zip $zip -Title $AddInTitle
}
finally {
    $zip.Dispose()
}

Write-Host "Injected $partName and set add-in title to '$AddInTitle' in $WorkbookPath"
