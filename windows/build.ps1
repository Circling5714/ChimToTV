# Đóng gói Côn Bằng TV cho Windows: một tệp .exe tự chạy (kèm .NET, không cần cài gì thêm ngoài WebView2 Runtime có sẵn
# trên Windows 10 / 11) → ..\dist\ConBangTV-Windows-<bản>.exe
# Số hiệu bản: <Version> trong ConBangTV\ConBangTV.csproj.
param([string]$Version = "")

$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $env:DOTNET_ROOT -and (Test-Path "$env:LOCALAPPDATA\Microsoft\dotnet\dotnet.exe")) {
    $env:DOTNET_ROOT = "$env:LOCALAPPDATA\Microsoft\dotnet"
    $env:PATH = "$env:DOTNET_ROOT;$env:PATH"
}
$env:DOTNET_CLI_TELEMETRY_OPTOUT = "1"
$env:DOTNET_NOLOGO = "1"

$proj = Join-Path $here "ConBangTV\ConBangTV.csproj"
if (-not $Version) { $Version = ([xml](Get-Content $proj)).Project.PropertyGroup.Version | Where-Object { $_ } | Select-Object -First 1 }
$short = $Version -replace '\.0$', ''

# Lõi Rust dùng chung với bản Linux (linux\core) → conbang_core.dll (linux\ffi): chạy test của lõi rồi dựng bản release; dự án
# WPF chép / gói DLL vào .exe. Cần Rust (rustup, bản MSVC) + Visual Studio Build Tools (C++).
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
if (-not (Test-Path $cargo)) { throw "Chưa có Rust: cài rustup (winget install Rustlang.Rustup) + Visual Studio Build Tools (C++)" }
$linux = Join-Path (Split-Path $here -Parent) "linux"
Push-Location $linux
try {
    Write-Host "Test lõi Rust…"
    & $cargo test -p conbang-core --quiet --color never
    if ($LASTEXITCODE -ne 0) { throw "Test lõi Rust lỗi" }
    Write-Host "Dựng lõi Rust (conbang_core.dll)…"
    & $cargo build -p conbang-ffi --release --quiet --color never
    if ($LASTEXITCODE -ne 0) { throw "Dựng lõi Rust lỗi" }
}
finally {
    Pop-Location
}
Write-Host "Chạy test…"
dotnet test (Join-Path $here "ConBangTV.Tests\ConBangTV.Tests.csproj") -c Release --nologo -v q
if ($LASTEXITCODE -ne 0) { throw "Test lỗi" }

$out = Join-Path $here "ConBangTV\bin\publish"
if (Test-Path $out) { Remove-Item $out -Recurse -Force }
Write-Host "Đóng gói bản $Version…"
dotnet publish $proj -c Release -r win-x64 --self-contained true -o $out -p:Version=$Version `
    -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true -p:EnableCompressionInSingleFile=true `
    -p:DebugType=none -p:GenerateDocumentationFile=false --nologo -v q
if ($LASTEXITCODE -ne 0) { throw "Đóng gói lỗi" }

$dist = Join-Path (Split-Path $here -Parent) "dist"
New-Item -ItemType Directory -Force $dist | Out-Null
$exe = Join-Path $dist "ConBangTV-Windows-$short.exe"
Copy-Item (Join-Path $out "ConBangTV.exe") $exe -Force
$info = Get-Item $exe
$sha = (Get-FileHash $exe -Algorithm SHA256).Hash.ToLower()
Write-Host ("Xong: {0} ({1:N1} MB)" -f $exe, ($info.Length / 1MB))
Write-Host "SHA-256: $sha"
Write-Host ("Bản trong tệp: {0}" -f $info.VersionInfo.ProductVersion)
