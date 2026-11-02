# Dựng ChimTo TV -> dist\ChimToTV-<phiên bản>.apk (và cài lên tivi nếu có -Install)
# Cách dùng:  .\build.ps1                      (chỉ dựng)
#             .\build.ps1 -Install <IP-tivi>    (dựng + cài qua adb rồi mở app, vd 192.168.1.50)
param([string]$Install)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$env:JAVA_HOME = "$env:USERPROFILE\.jdks\jdk-17.0.20.1+1"
$sdk = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_HOME = $sdk
$adb = "$sdk\platform-tools\adb.exe"

# Khoá ký riêng của app (tạo một lần, giữ lại để các bản sau cài đè được)
if (-not (Test-Path keys\ky.properties)) {
    New-Item -ItemType Directory -Force keys | Out-Null
    $pass = -join ((48..57) + (65..90) + (97..122) | Get-Random -Count 20 | ForEach-Object { [char]$_ })
    & "$env:JAVA_HOME\bin\keytool.exe" -genkeypair -keystore keys\chimto.jks -alias chimto -keyalg RSA -keysize 2048 `
        -validity 10000 -storepass $pass -keypass $pass -dname "CN=ChimTo TV" | Out-Null
    if ($LASTEXITCODE) { throw "Tạo khoá ký thất bại" }
    "storeFile=keys/chimto.jks`nstorePassword=$pass`nkeyAlias=chimto`nkeyPassword=$pass" | Set-Content -Encoding ascii keys\ky.properties
    Write-Host "Đã tạo khoá ký keys\chimto.jks (giữ lại thư mục keys!)" -ForegroundColor Yellow
}

.\gradlew.bat --console=plain :app:assembleRelease
if ($LASTEXITCODE) { throw "Dựng APK thất bại" }

$ver = (Select-String -Path app\build.gradle.kts -Pattern 'versionName = "(.+)"').Matches[0].Groups[1].Value
New-Item -ItemType Directory -Force dist | Out-Null
$apk = "dist\ChimToTV-$ver.apk"
Copy-Item app\build\outputs\apk\release\app-release.apk $apk -Force
Write-Host "OK -> $apk" -ForegroundColor Green

if ($Install) {
    $dev = if ($Install -match ":") { $Install } else { "${Install}:5555" }
    & $adb connect $dev | Out-Host
    & $adb -s $dev install -r $apk | Out-Host
    if ($LASTEXITCODE) { throw "Cài lên tivi thất bại" }
    & $adb -s $dev shell am start -n vn.chimto.tv/.MainActivity | Out-Host
}
