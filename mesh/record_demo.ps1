# OASIS Mesh 3-platform demo recorder - single take, no edit, ~120s
# Layout 1920x1080:
#   [scrcpy phone 0,0  480x1080] [webots 480,0  1440x540]
#                                 [terminal 480,540  1440x540]

param(
  [int]$DurationSec = 300,
  [int]$WarmupSec = 60
)

$ErrorActionPreference = 'Continue'   # ADB writes warnings to stderr, don't abort on them

$FFMPEG = "C:\Users\pc\AppData\Local\Microsoft\WinGet\Packages\Gyan.FFmpeg_Microsoft.Winget.Source_8wekyb3d8bbwe\ffmpeg-8.1-full_build\bin\ffmpeg.exe"
$SCRCPY = "C:\Users\pc\AppData\Local\Microsoft\WinGet\Packages\Genymobile.scrcpy_Microsoft.Winget.Source_8wekyb3d8bbwe\scrcpy-win64-v3.3.4\scrcpy.exe"
$ADB    = "C:\Users\pc\AppData\Local\Temp\platform-tools\adb.exe"
$WEBOTS = "C:\Program Files\Webots\msys64\mingw64\bin\webots.exe"
$WORLD  = "c:\dev\oasis\webots\worlds\oasis_factory.wbt"
$OUT    = "c:\dev\oasis\mesh\demo-mesh-3plat.mp4"

# ---- 0. Cleanup prior instances + MINIMIZE distracting apps for clean recording ----
Get-Process scrcpy,ffmpeg,webots,webots-bin,drone_bridge -ErrorAction SilentlyContinue | Stop-Process -Force
# Minimize (not kill) VSCode + browsers — user keeps their work open
Add-Type -Name WinUtil -Namespace U -MemberDefinition '
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);'
foreach ($p in Get-Process Code,chrome,msedge,firefox,brave -ErrorAction SilentlyContinue) {
  if ($p.MainWindowHandle -ne [IntPtr]::Zero) {
    [U.WinUtil]::ShowWindow($p.MainWindowHandle, 6) | Out-Null   # SW_MINIMIZE
  }
}
# Hide taskbar (auto-hide) for the duration
$taskbarReg = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StuckRects3'
$prevSettings = (Get-ItemProperty -Path $taskbarReg -Name Settings -ErrorAction SilentlyContinue).Settings
if ($prevSettings) {
  $newSettings = $prevSettings.Clone()
  $newSettings[8] = 3   # auto-hide
  Set-ItemProperty -Path $taskbarReg -Name Settings -Value $newSettings
  Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue  # restart taskbar
  Start-Sleep -Seconds 2
}
Start-Sleep -Seconds 1

# ---- 1. Verify phone daemon ----
$pid_phone = & $ADB shell "pgrep -f oasis-rt" 2>$null
if (-not $pid_phone) {
  Write-Host "ERROR: phone daemon not running. Open Termux + run: bash /sdcard/launch-oasis.sh" -ForegroundColor Red
  exit 1
}
Write-Host "Phone daemon PID: $pid_phone" -ForegroundColor Green

# ---- 1.5 Take control of phone: wake, open Termux, start tail -f live log ----
Write-Host "Taking control of phone: wake + Termux + tail -f live log..."
& $ADB shell "input keyevent KEYCODE_WAKEUP" 2>&1 | Out-Null
Start-Sleep -Milliseconds 800
& $ADB shell "input keyevent KEYCODE_MENU" 2>&1 | Out-Null
Start-Sleep -Milliseconds 500
& $ADB shell "am start -n com.termux/com.termux.app.TermuxActivity" 2>&1 | Out-Null
Start-Sleep -Seconds 2
& $ADB shell "input text 'clear'" 2>&1 | Out-Null
& $ADB shell "input keyevent KEYCODE_ENTER" 2>&1 | Out-Null
Start-Sleep -Milliseconds 500
& $ADB shell "input text 'tail%s-f%s/sdcard/oasis-4h-log.txt'" 2>&1 | Out-Null
& $ADB shell "input keyevent KEYCODE_ENTER" 2>&1 | Out-Null
Start-Sleep -Seconds 1
Write-Host "Phone now showing live OASIS daemon log" -ForegroundColor Green

# ---- 2. Launch scrcpy: phone mirror, top-left strip ----
Write-Host "Launching scrcpy phone mirror..."
Start-Process -FilePath $SCRCPY -ArgumentList @(
  "--window-title=PHONE",
  "--window-borderless",
  "--window-x=0",
  "--window-y=0",
  "--window-width=540",
  "--window-height=1080",
  "--max-fps=15",
  "--no-audio"
) -WindowStyle Normal
Start-Sleep -Seconds 3

# ---- 3. Cleanup mesh dir + Webots logs ----
Remove-Item "c:\dev\oasis\mesh\shared\*" -Force -ErrorAction SilentlyContinue
Remove-Item "c:\dev\oasis\webots\factory_*.log" -Force -ErrorAction SilentlyContinue
Remove-Item "c:\dev\oasis\webots\factory_shared\*" -Force -ErrorAction SilentlyContinue

# ---- 4. Launch Webots IN BACKGROUND — minimize+no-rendering, invisible from capture ----
Write-Host "Launching Webots (hidden, no GUI)..."
$env:OASIS_MESH_PEERS = "phone,pc-node"
Start-Process -FilePath $WEBOTS -ArgumentList @(
  "--mode=fast",
  "--minimize",
  "--no-rendering",
  "--batch",
  "$WORLD"
) -WindowStyle Hidden
Start-Sleep -Seconds 6

# ---- 5. Position Webots window via Win32 API (kept for safety: move offscreen) ----
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win32 {
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);
  [DllImport("user32.dll", CharSet=CharSet.Auto)] public static extern int EnumWindows(EnumWindowsProc enumFunc, int lParam);
  [DllImport("user32.dll", CharSet=CharSet.Auto)] public static extern int GetWindowText(IntPtr hWnd, System.Text.StringBuilder lpString, int nMaxCount);
  public delegate bool EnumWindowsProc(IntPtr hWnd, int lParam);
}
"@ -ErrorAction SilentlyContinue

# Find Webots window by partial title and move it
$webotsHandle = [IntPtr]::Zero
$cb = [Win32+EnumWindowsProc]{
  param($hWnd, $lParam)
  $sb = New-Object System.Text.StringBuilder 256
  [Win32]::GetWindowText($hWnd, $sb, 256) | Out-Null
  if ($sb.ToString() -match "Webots|oasis_factory") {
    $script:webotsHandle = $hWnd
    return $false
  }
  return $true
}
[Win32]::EnumWindows($cb, 0) | Out-Null
if ($webotsHandle -ne [IntPtr]::Zero) {
  # Move offscreen + small size as belt-and-suspenders: invisible regardless of --minimize behaviour
  [Win32]::SetWindowPos($webotsHandle, [IntPtr]::Zero, -3000, -3000, 100, 100, 0x0040) | Out-Null
  Write-Host "Webots moved off-screen" -ForegroundColor Green
}

# ---- 6. Launch a terminal showing live mesh log, bottom-right ----
$logCmd = "title MESH-LIVE && echo OASIS Mesh 3-Platform Demo && echo Phone PID=$pid_phone && echo --- && powershell -NoExit -Command `"Get-Content -Path 'c:\dev\oasis\mesh\logs\demo.log' -Wait -Tail 30`""
Start-Process cmd.exe -ArgumentList "/k", $logCmd -WindowStyle Normal
Start-Sleep -Seconds 2

# Find cmd window and reposition
$cmdHandle = [IntPtr]::Zero
$cb2 = [Win32+EnumWindowsProc]{
  param($hWnd, $lParam)
  $sb = New-Object System.Text.StringBuilder 256
  [Win32]::GetWindowText($hWnd, $sb, 256) | Out-Null
  if ($sb.ToString() -match "MESH-LIVE") {
    $script:cmdHandle = $hWnd
    return $false
  }
  return $true
}
[Win32]::EnumWindows($cb2, 0) | Out-Null
if ($cmdHandle -ne [IntPtr]::Zero) {
  [Win32]::SetWindowPos($cmdHandle, [IntPtr]::Zero, 540, 0, 1380, 1080, 0x0040) | Out-Null
  Write-Host "Terminal positioned: 540,0 1380x1080 (full height)" -ForegroundColor Green
}

# ---- 6.5 Fill any leftover desktop area with a black topmost overlay (cover VSCode/icons) ----
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$blackForm = New-Object System.Windows.Forms.Form
$blackForm.FormBorderStyle = 'None'
$blackForm.WindowState = 'Maximized'
$blackForm.BackColor = [System.Drawing.Color]::Black
$blackForm.TopMost = $false  # not topmost — sits behind the scrcpy/webots/cmd windows
$blackForm.ShowInTaskbar = $false
# Show on a separate runspace
$runspace = [runspacefactory]::CreateRunspace()
$runspace.Open()
$ps = [powershell]::Create()
$ps.Runspace = $runspace
$ps.AddScript({
  param($f)
  Add-Type -AssemblyName System.Windows.Forms
  [System.Windows.Forms.Application]::Run($f)
}).AddArgument($blackForm) | Out-Null
$null = $ps.BeginInvoke()
Start-Sleep -Seconds 1
# Send black form to BACK so the demo windows stay on top
$blackHandle = $blackForm.Handle
[Win32]::SetWindowPos($blackHandle, [IntPtr]1, 0, 0, 1920, 1080, 0x0010) | Out-Null  # HWND_BOTTOM

# ---- 7. Launch mesh sync background ----
Write-Host "Starting mesh script..."
$meshLog = "c:\dev\oasis\mesh\logs\demo.log"
New-Item -ItemType Directory -Path "c:\dev\oasis\mesh\logs" -Force | Out-Null
"" | Out-File $meshLog
$meshJob = Start-Job -ScriptBlock {
  param($logPath)
  & "C:\Program Files\Git\bin\bash.exe" -c "c:/dev/oasis/mesh/run_mesh_with_phone.sh 2>&1" | Tee-Object -FilePath $logPath
} -ArgumentList $meshLog

# ---- 7.5 Warmup: let mesh produce digests + cross-platform exchange before recording ----
if ($WarmupSec -gt 0) {
  Write-Host "WARMUP $WarmupSec sec (mesh runs, digests accumulate, no recording yet)..." -ForegroundColor Yellow
  Start-Sleep -Seconds $WarmupSec
  Write-Host "Warmup complete. Starting recording..."
}

# ---- 8. Start ffmpeg recording (gdigrab full desktop) ----
Write-Host "RECORDING for $DurationSec seconds -> $OUT" -ForegroundColor Cyan
Remove-Item $OUT -Force -ErrorAction SilentlyContinue
$ffmpegLog = "c:\dev\oasis\mesh\logs\ffmpeg.log"
$ffArgs = @(
  "-y", "-f", "gdigrab", "-framerate", "12",
  "-offset_x", "0", "-offset_y", "0",
  "-video_size", "1920x1080", "-i", "desktop",
  "-t", "$DurationSec",
  "-c:v", "libx264", "-preset", "fast", "-crf", "23", "-pix_fmt", "yuv420p",
  "$OUT"
)
$ffProc = Start-Process -FilePath $FFMPEG -ArgumentList $ffArgs `
  -RedirectStandardError $ffmpegLog -RedirectStandardOutput "$ffmpegLog.out" `
  -PassThru -WindowStyle Hidden
Write-Host "ffmpeg PID: $($ffProc.Id) - recording..."
$ffProc.WaitForExit(($DurationSec + 10) * 1000)
Write-Host "RECORDING COMPLETE" -ForegroundColor Cyan

# ---- 9. Cleanup ----
Stop-Job $meshJob -ErrorAction SilentlyContinue
Remove-Job $meshJob -Force -ErrorAction SilentlyContinue
Get-Process scrcpy,webots,webots-bin,drone_bridge -ErrorAction SilentlyContinue | Stop-Process -Force

# ---- 10. Report ----
if (Test-Path $OUT) {
  $size = (Get-Item $OUT).Length / 1MB
  Write-Host ""
  Write-Host "=== DELIVRABLE ===" -ForegroundColor Green
  Write-Host "  $OUT"
  Write-Host "  Size: $([math]::Round($size,1)) MB"
  Write-Host "  Duration: $DurationSec s (after $WarmupSec s warmup)"
  Write-Host "  Layout: phone scrcpy 540x1080 (left, live tail -f) | mesh log 1380x1080 (right, full height) | Webots invisible (background)"
  Write-Host ""
  Write-Host "Phone daemon still running. Kill manually via Termux: bash /sdcard/kill-oasis.sh"
} else {
  Write-Host "ERROR: video file not created" -ForegroundColor Red
}
