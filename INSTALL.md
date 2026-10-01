# Installing `cvxx`

`cvxx` ships as an unsigned 64-bit Excel add-in pair — `cvxx.xll` and
`cvxx.xlam` — distributed as a single `cvxx-{version}.zip` archive on the
[GitHub Releases](https://github.com/QuantAnalyticsTorch/cvxx/releases) page
from `v1.0.0` onward. No installer, no code signing, no network access is
required at any point.

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
("Mark of the Web"). Excel will otherwise refuse to load the add-ins, or
open them in Protected View. Unblock everything in one step:

```powershell
Get-ChildItem -Path C:\Tools\cvxx -Recurse | Unblock-File
```

Alternatively, right-click each file in File Explorer → **Properties** →
check **Unblock** → **OK**.

## 4. Trust the add-ins in Excel

Because the add-ins are not code-signed, Excel needs an explicit add-in
registration rather than relying on a publisher signature:

1. In Excel: **File** → **Options** → **Add-ins**.
2. At the bottom, set **Manage** to **Excel Add-ins**, then click **Go…**.
3. Click **Browse…**, select `cvxx.xll`, and click **OK**. Repeat for
   `cvxx.xlam`. Both should now be checked in the **Add-Ins available** list.
4. Click **OK** to load them. A `cvxx` ribbon tab should appear.

If Excel still blocks the files as unsafe, double-check step 3 (unblocking)
was applied to every file, including the ones inside `docs\`.

## 5. Try it out

- Open an example workbook from `docs\examples\`, or use the ribbon's
  **Examples** button.
- Browse the function reference via the ribbon's **Help** button (opens
  `docs\html\index.html` from disk — no network access needed), or online
  under [`docs/`](https://github.com/QuantAnalyticsTorch/cvxx/tree/main/docs).

## Uninstalling

Repeat step 2 above and uncheck both add-ins, or delete the extracted
folder; `cvxx` does not write anywhere else on disk or touch the registry.
