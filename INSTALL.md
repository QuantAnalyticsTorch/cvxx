# Installing `cvxx`

`cvxx` ships as an unsigned 64-bit Excel add-in pair — `cvxx.xll` and
`cvxx.xlam` — distributed as a single `cvxx-{version}.zip` archive on the
[GitHub Releases](https://github.com/QuantAnalyticsTorch/cvxx/releases) page
from `v1.0.0` onward. No MSI/EXE installer, no code signing, no network
access is required at any point; the archive also bundles an optional
`install.ps1`/`install.bat` helper that automates the file-unblocking step
below.

## 1. Requirements

- 64-bit Excel 2016 or later, on Windows.
- `x86_64-pc-windows-msvc` only; 32-bit Excel is not supported.

## 2. Download and extract

1. Download `cvxx-{version}.zip` from the release's **Assets**.
2. Extract it to a folder you control, e.g. `C:\Tools\cvxx\`. Keep
   `cvxx.xll`, `cvxx.xlam`, and `docs\` together in the same folder — the
   ribbon's **Help** and **Examples** buttons use paths relative to
   `cvxx.xlam`'s own location.
3. (Optional) Verify the download with `SHA256SUMS.txt`:

   ```powershell
   cd C:\Tools\cvxx
   $expected = Get-Content SHA256SUMS.txt
   Get-FileHash -Algorithm SHA256 cvxx.xll, cvxx.xlam
   ```

   Compare the printed hashes against the matching lines in
   `SHA256SUMS.txt`.

## 3. Unblock the files

Windows marks files extracted from a downloaded zip as untrusted
("Mark of the Web", or MOTW). If left in place, Excel will refuse to even
*load* `cvxx.xlam` as an add-in — because it contains a VBA project, a
MOTW-tagged copy trips Excel's "Block macros from running in Office files
from the internet" policy (enabled by default since 2022), which is
stricter than the usual Protected View banner and cannot be dismissed by
clicking "Enable Content".

**Recommended: run the bundled installer helper.** Double-click
`install.bat` in the extracted folder (or run `.\install.ps1` in
PowerShell). It unblocks every file in the folder, including itself, and
optionally registers the folder as an Excel **Trusted Location** if you
answer "y" when prompted — useful if Group Policy still blocks macros from
internet-sourced files after unblocking. It never touches Excel's add-in
registration (step 4 below is still a one-time manual step), and it is
safe to re-run. Windows SmartScreen may show a one-time "Windows protected
your PC" prompt the first time you run an unsigned `.bat`/`.ps1`; click
**More info** → **Run anyway** to proceed.

**Manual alternative**, if you'd rather not run a script:

```powershell
Get-ChildItem -Path C:\Tools\cvxx -Recurse | Unblock-File
```

Alternatively, right-click each file in File Explorer → **Properties** →
check **Unblock** → **OK**. Unblock **after extracting**, not before
(unblocking the `.zip` itself does not reliably propagate to the files
extracted from it). To check whether a specific file is still blocked:

```powershell
Get-Item C:\Tools\cvxx\cvxx.xlam -Stream Zone.Identifier -ErrorAction SilentlyContinue
```

If this prints a `Zone.Identifier` stream, the file is still blocked; if it
prints nothing, the file is unblocked.

## 4. Trust the add-ins in Excel

Because the add-ins are not code-signed, Excel needs an explicit add-in
registration rather than relying on a publisher signature:

1. In Excel: **File** → **Options** → **Add-ins**.
2. At the bottom, set **Manage** to **Excel Add-ins**, then click **Go…**.
3. Click **Browse…**, select `cvxx.xll`, and click **OK**. Repeat for
   `cvxx.xlam`. Both should now be checked in the **Add-Ins available** list.
4. Click **OK** to load them. A `cvxx` ribbon tab should appear.

### If Excel reports "blocked macros" or `cvxx.xlam` fails to load

1. Re-run `install.bat`/`install.ps1` (or step 3's manual commands) and
   confirm **every** file under the extracted folder, including
   `cvxx.xlam` itself, has no `Zone.Identifier` stream left (a stray
   blocked file is the most common cause).
2. If your organization enforces macro security via Group Policy, unblocking
   may not be enough. Run `install.ps1 -TrustedLocation` (or answer "y" to
   `install.bat`'s prompt) to register the folder as a **Trusted Location**,
   which bypasses the MOTW/VBA check entirely. To do this by hand instead:
   **File** → **Options** → **Trust Center** → **Trust Center Settings…** →
   **Trusted Locations** → **Add new location…**, browse to your `cvxx`
   folder (e.g. `C:\Tools\cvxx\`), check **Subfolders of this location are
   also trusted**, then **OK** and restart Excel.

## 5. Try it out

- Open an example workbook from `docs\examples\`, or use the ribbon's
  **Examples** button.
- Browse the function reference via the ribbon's **Help** button (opens
  `docs\html\index.html` from disk — no network access needed), or online
  under [`docs/`](https://github.com/QuantAnalyticsTorch/cvxx/tree/main/docs).

## Uninstalling

Repeat step 2 above and uncheck both add-ins, or delete the extracted
folder; `cvxx` does not write anywhere else on disk. If you used
`install.ps1 -TrustedLocation` (or answered "y" in `install.bat`), also
remove the matching entry under **File** → **Options** → **Trust Center**
→ **Trust Center Settings…** → **Trusted Locations**; otherwise `cvxx`
does not touch the registry at all.
