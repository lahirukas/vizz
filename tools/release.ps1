# Builds, optionally signs, and zips a release into dist\ without GitHub Actions.
# Usage: powershell -NoProfile -File tools\release.ps1 [-Pfx $HOME\vizz-signing.pfx]
param([string]$Pfx = "$HOME\vizz-signing.pfx")
$ErrorActionPreference = "Stop"
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
cargo build --release -p vizz-app
if ($LASTEXITCODE -ne 0) { throw "build failed" }
$dir = "dist\vizz-$version-windows-x64"
Remove-Item -Recurse -Force $dir, "$dir.zip", "$dir.zip.sha256" -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item target\release\vizz.exe, README.md, LICENSE, CHANGELOG.md $dir
if (Test-Path $Pfx) {
    $secure = Read-Host "PFX password" -AsSecureString
    $plain = [Runtime.InteropServices.Marshal]::PtrToStringBSTR([Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure))
    $signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" | Sort-Object FullName | Select-Object -Last 1
    & $signtool.FullName sign /fd SHA256 /td SHA256 /tr http://timestamp.digicert.com /f $Pfx /p $plain /d "Vizz" /du "https://github.com/lahirukas/vizz" "$dir\vizz.exe"
    if ($LASTEXITCODE -ne 0) { throw "signing failed" }
} else {
    Write-Warning "No certificate at $Pfx; vizz.exe is unsigned."
}
Compress-Archive -Path $dir -DestinationPath "$dir.zip"
(Get-FileHash "$dir.zip" -Algorithm SHA256).Hash | Out-File "$dir.zip.sha256" -Encoding ascii
Write-Host "Release ready: $dir.zip"
