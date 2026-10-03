@echo off
setlocal

echo cvxx install helper
echo.
echo This unblocks every file in this folder (removes Windows' "Mark of
echo the Web"), which is required before Excel will load cvxx.xlam.
echo.
set /p TRUST="Also register this folder as an Excel Trusted Location? [y/N]: "

if /i "%TRUST%"=="y" (
    set "TRUSTFLAG=-TrustedLocation"
) else (
    set "TRUSTFLAG="
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1" %TRUSTFLAG%

echo.
pause
