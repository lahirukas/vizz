# Measure vizz working set and CPU over a window, then make sure it is gone.
# Usage: powershell -NoProfile -File tools\perf.ps1 -Exe target\release\vizz.exe -AppArgs "--source loopback --console" [-Warmup 3] [-Window 10]
param(
    [Parameter(Mandatory)] [string]$Exe,
    [string]$AppArgs = "--source loopback --console",
    [int]$Warmup = 3,
    [int]$Window = 10
)
$ErrorActionPreference = 'Stop'
$total = $Warmup + $Window + 3
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = (Resolve-Path $Exe).Path
$psi.Arguments = "$AppArgs --exit-after $total"
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.CreateNoWindow = $true
$p = [System.Diagnostics.Process]::Start($psi)
try {
    $null = $p.StandardOutput.ReadToEndAsync()
    Start-Sleep -Seconds $Warmup
    $p.Refresh()
    $cpu0 = $p.TotalProcessorTime.TotalMilliseconds
    $t0 = Get-Date
    $peakWs = 0
    for ($i = 0; $i -lt $Window * 2; $i++) {
        Start-Sleep -Milliseconds 500
        $p.Refresh()
        $peakWs = [Math]::Max($peakWs, $p.WorkingSet64)
    }
    $p.Refresh()
    $cpu1 = $p.TotalProcessorTime.TotalMilliseconds
    $wall = ((Get-Date) - $t0).TotalMilliseconds
    $cores = [Environment]::ProcessorCount
    [pscustomobject]@{
        ws_mb        = [Math]::Round($p.WorkingSet64 / 1MB, 1)
        peak_ws_mb   = [Math]::Round($peakWs / 1MB, 1)
        private_mb   = [Math]::Round($p.PrivateMemorySize64 / 1MB, 1)
        cpu_pct      = [Math]::Round(100 * ($cpu1 - $cpu0) / $wall / $cores, 2)
        cpu_pct_core = [Math]::Round(100 * ($cpu1 - $cpu0) / $wall, 2)
        threads      = $p.Threads.Count
    } | Format-List | Out-String
} finally {
    if (-not $p.HasExited) { $p.Kill() }
    Get-Process vizz -ErrorAction SilentlyContinue | Stop-Process -Force
}
