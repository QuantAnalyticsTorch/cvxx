Attribute VB_Name = "modRibbon"
Option Explicit

' Ribbon callback module for cvxx.xlam.
'
' This file is NOT compiled/imported automatically; it is a plain-text export
' kept under xlam/source/ purely so ribbon/VBA changes are reviewable in
' diffs. To apply changes, import this module into the VBA project behind
' cvxx.xlam via the VBE (Alt+F11 -> File -> Import File).
'
' Callback names below must match the onAction/getContent attributes in
' customUI.xml exactly.

Private gRibbon As IRibbonUI

Private Const SUPPORT_EMAIL As String = "quant.analytics.torch@gmail.com"
Private Const ISSUE_URL As String = "https://github.com/QuantAnalyticsTorch/cvxx/issues"

' --- Ribbon lifecycle -------------------------------------------------

Public Sub OnRibbonLoad(ribbon As IRibbonUI)
    Set gRibbon = ribbon
End Sub

' Folder containing docs/examples and docs/html. In the shipped release
' archive, cvxx.xlam sits next to docs/ directly. In this repo checkout,
' cvxx.xlam lives under assets/ while docs/ is one level up at the repo
' root, so fall back to the parent folder when docs/ isn't a sibling.
Private Function DocsRoot() As String
    Dim here As String
    here = ThisWorkbook.Path

    If Len(Dir(here & "\docs", vbDirectory)) > 0 Then
        DocsRoot = here
    Else
        DocsRoot = Left$(here, InStrRev(here, "\") - 1)
    End If
End Function

' Diagnostic macro, callable standalone from Excel's Alt+F8 macro list (no
' arguments, so it shows up there unlike the ribbon callbacks). Reports the
' paths DocsRoot() resolves and whether they actually exist, to debug why
' the Examples/Docs ribbon buttons report "not found".
Public Sub Test_DocsRoot()
    Dim root As String
    Dim examplesFolder As String
    Dim docsIndex As String

    root = DocsRoot()
    examplesFolder = root & "\docs\examples\"
    docsIndex = root & "\docs\html\index.html"

    MsgBox "ThisWorkbook.Path: " & ThisWorkbook.Path & vbCrLf & _
           "DocsRoot(): " & root & vbCrLf & vbCrLf & _
           "Examples folder: " & examplesFolder & vbCrLf & _
           "  exists: " & CStr(Len(Dir(examplesFolder, vbDirectory)) > 0) & vbCrLf & _
           "  *.xls* found: " & Dir(examplesFolder & "*.xls*") & vbCrLf & vbCrLf & _
           "Docs index: " & docsIndex & vbCrLf & _
           "  exists: " & CStr(Len(Dir(docsIndex)) > 0), _
           vbInformation, "cvxx DocsRoot diagnostics"
End Sub

' --- Examples -----------------------------------------------------------

Public Sub Examples_GetContent(control As IRibbonControl, ByRef content As Variant)
    Dim folder As String
    Dim fileName As String
    Dim xml As String

    folder = DocsRoot() & "\docs\examples\"
    xml = "<menu xmlns=""http://schemas.microsoft.com/office/2009/07/customui"">"

    If Len(Dir(folder, vbDirectory)) = 0 Then
        xml = xml & "<button id=""noExamplesFolder"" label=""docs/examples not found"" enabled=""false"" />"
    Else
        fileName = Dir(folder & "*.xls*")
        If Len(fileName) = 0 Then
            xml = xml & "<button id=""noExamples"" label=""No example workbooks found"" enabled=""false"" />"
        End If

        Do While Len(fileName) > 0
            xml = xml & "<button id=""ex_" & EscapeXmlAttr(fileName) & """" & _
                        " label=""" & EscapeXmlAttr(fileName) & """" & _
                        " onAction=""Examples_OnAction""" & _
                        " tag=""" & EscapeXmlAttr(folder & fileName) & """ />"
            fileName = Dir()
        Loop
    End If

    xml = xml & "</menu>"
    content = xml
End Sub

Public Sub Examples_OnAction(control As IRibbonControl)
    On Error GoTo Fail
    Workbooks.Open fileName:=control.Tag
    Exit Sub
Fail:
    MsgBox "Could not open example workbook:" & vbCrLf & control.Tag & vbCrLf & Err.Description, _
           vbExclamation, "cvxx"
End Sub

' --- Documentation --------------------------------------------------------

Public Sub Docs_OnAction(control As IRibbonControl)
    Dim indexPath As String
    indexPath = DocsRoot() & "\docs\html\index.html"

    If Len(Dir(indexPath)) = 0 Then
        MsgBox "Documentation was not found at:" & vbCrLf & indexPath, vbExclamation, "cvxx"
        Exit Sub
    End If

    ThisWorkbook.FollowHyperlink indexPath
End Sub

' --- Support & feedback (optional) -----------------------------------------

Public Sub Support_OnAction(control As IRibbonControl)
    Dim subjectLine As String
    Dim bodyLines As String

    subjectLine = "cvxx support request"
    bodyLines = "Describe your question or problem here." & vbCrLf & vbCrLf & _
                "Excel version: " & Application.Version

    ThisWorkbook.FollowHyperlink "mailto:" & SUPPORT_EMAIL & _
        "?subject=" & UrlEncode(subjectLine) & _
        "&body=" & UrlEncode(bodyLines)
End Sub

Public Sub Feedback_OnAction(control As IRibbonControl)
    ThisWorkbook.FollowHyperlink ISSUE_URL
End Sub

' --- Helpers ---------------------------------------------------------------

Private Function EscapeXmlAttr(ByVal s As String) As String
    s = Replace(s, "&", "&amp;")
    s = Replace(s, """", "&quot;")
    s = Replace(s, "<", "&lt;")
    s = Replace(s, ">", "&gt;")
    EscapeXmlAttr = s
End Function

Private Function UrlEncode(ByVal s As String) As String
    Dim i As Long
    Dim code As Integer
    Dim ch As String
    Dim result As String

    For i = 1 To Len(s)
        ch = Mid$(s, i, 1)
        code = AscW(ch)
        Select Case code
            Case 48 To 57, 65 To 90, 97 To 122 ' 0-9 A-Z a-z
                result = result & ch
            Case 45, 46, 95, 126 ' - . _ ~
                result = result & ch
            Case 32 ' space
                result = result & "%20"
            Case 13 ' CR
                result = result & "%0D"
            Case 10 ' LF
                result = result & "%0A"
            Case Else
                result = result & "%" & Right$("0" & Hex$(code), 2)
        End Select
    Next i

    UrlEncode = result
End Function
