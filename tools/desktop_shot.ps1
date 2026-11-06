# Capture the whole virtual desktop to a PNG (no installs needed).
# Usage: powershell -NoProfile -File tools\desktop_shot.ps1 [-Out shots\desk.png] [-DelayMs 0]
# Keeps at most the 10 newest PNGs in the output folder.
param(
    [string]$Out = "shots\desktop_$(Get-Date -Format yyyyMMdd_HHmmss).png",
    [int]$DelayMs = 0
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @'
using System.Runtime.InteropServices;
public static class Dpi { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware(); }
'@
[void][Dpi]::SetProcessDPIAware()
if ($DelayMs -gt 0) { Start-Sleep -Milliseconds $DelayMs }

$b = [System.Windows.Forms.SystemInformation]::VirtualScreen
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
try {
    $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
    $dir = Split-Path -Parent $Out
    if ($dir) { New-Item -ItemType Directory -Force $dir | Out-Null }
    $full = [System.IO.Path]::GetFullPath((Join-Path (Get-Location) $Out))
    $bmp.Save($full, [System.Drawing.Imaging.ImageFormat]::Png)
    Write-Output $full
} finally {
    $g.Dispose(); $bmp.Dispose()
}
if ($dir) {
    Get-ChildItem $dir -Filter *.png | Sort-Object LastWriteTime -Descending | Select-Object -Skip 10 | Remove-Item -Force
}
