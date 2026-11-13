# Dựng bản Linux trên máy ảo Ubuntu thử (điều khiển bằng E:\DEV_CODE\_May_Test_App\ubuntu.ps1 — xem README-Ubuntu.md ở đó).
#
#   .\linux\build.ps1                 gửi mã nguồn đang sửa sang máy Ubuntu, dựng bản thử (debug)
#   .\linux\build.ps1 -Test           + chạy test của lõi (cargo test)
#   .\linux\build.ps1 -Run            + mở app trên màn hình máy Ubuntu (tắt bản đang chạy trước)
#   .\linux\build.ps1 -Release -Deb   dựng bản chính + đóng gói dist\linux\ConBangTV-Linux-<bản>-thu.deb (bản phát hành thật
#                                     dựng trong Ubuntu 22.04 để chạy được trên máy cũ — xem README «Bản Linux»)
#   .\linux\build.ps1 -SyncOnly       chỉ gửi mã nguồn
#   .\linux\build.ps1 -CoreOnly -Test chỉ dựng + test phần lõi (linux/core, không cần GTK) — nhanh
#   -Slot <tên>                       dùng thư mục riêng ~/TestLab/ConBangTV-<tên> (nhiều người / nhiều việc dựng song song)
#
# Trên máy Ubuntu: ~/TestLab/ConBangTV[-<slot>]/src (mã nguồn, xoá và chép lại mỗi lần), …/target (thư mục dựng của
# cargo, giữ lại để dựng lần sau nhanh).
param([switch]$Release, [switch]$Test, [switch]$Run, [switch]$Deb, [switch]$SyncOnly, [switch]$CoreOnly, [string]$TestFilter = '', [string]$Slot = '')
$ErrorActionPreference = 'Stop'
$toolkit = if ($env:CONBANG_UBUNTU_TOOLKIT) { $env:CONBANG_UBUNTU_TOOLKIT } else { 'E:\DEV_CODE\_May_Test_App\ubuntu.ps1' }
. $toolkit
$repo = Split-Path $PSScriptRoot -Parent
$work = if ($Slot) { "TestLab/ConBangTV-$Slot" } else { 'TestLab/ConBangTV' }
$tmp = Join-Path $env:TEMP ("cbtv-linux-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $tmp | Out-Null
try {
    Push-Location $repo
    $files = @(git ls-files -co --exclude-standard -- linux app/src/main/assets design/logo-512.png |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
    # Mã hồ sơ NextDNS (DNS mã hoá) — tệp không có trên git; không có thì bản dựng bỏ NextDNS
    if (Test-Path 'keys\nextdns.txt') { $files += 'keys/nextdns.txt' }
    [IO.File]::WriteAllLines((Join-Path $tmp 'files.txt'), $files, (New-Object Text.UTF8Encoding $false))
    & "$env:SystemRoot\System32\tar.exe" -cf (Join-Path $tmp 'src.tar') -T (Join-Path $tmp 'files.txt')
    if ($LASTEXITCODE) { throw 'Đóng gói mã nguồn lỗi' }
    Pop-Location
    Ub-Ssh "mkdir -p ~/$work" | Out-Null
    Ub-Put (Join-Path $tmp 'src.tar') "$work/src.tar" | Out-Null
    Ub-Ssh ("cd ~/$work && rm -rf src && mkdir src && tar -xf src.tar -C src && rm -f src.tar && " + 'echo "Đã gửi $(find src -type f | wc -l) tệp"')
} finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
if ($SyncOnly) { return }

$profile = if ($Release) { 'release' } else { 'debug' }
$flag = if ($Release) { '--release' } else { '' }
$pkg = if ($CoreOnly) { '-p conbang-core' } else { '' }
$sw = [Diagnostics.Stopwatch]::StartNew()
$out = Ub-Ssh "cd ~/$work/src/linux && export CARGO_TARGET_DIR=~/$work/target && cargo build $flag $pkg --color never 2>&1 | grep -vE '^\s+(Compiling|Downloaded|Downloading|Locking|Adding|Updating)\b' | tail -60; echo ""BUILD_EXIT=`${PIPESTATUS[0]}"""
$out | Where-Object { $_ -notmatch '^BUILD_EXIT=' -and $_ -notmatch 'unused_dependencies|consider removing the dep|^warning: unused dependency|^\s*\|\s*$|^\s+--> \S*Cargo.toml' } | Out-Host
if (($out | Select-Object -Last 1) -ne 'BUILD_EXIT=0') { throw "Dựng lỗi ($([int]$sw.Elapsed.TotalSeconds) s)" }
"Dựng xong bản $profile trong $([int]$sw.Elapsed.TotalSeconds) s"

if ($Test) {
    $t = Ub-Ssh "cd ~/$work/src/linux && export CARGO_TARGET_DIR=~/$work/target && timeout 600 cargo test -p conbang-core --color never $TestFilter 2>&1 | grep -vE '^\s+(Compiling|Running|Doc-tests)\b|^running 0 tests|^$' | tail -80; echo ""TEST_EXIT=`${PIPESTATUS[0]}"""
    $t | Where-Object { $_ -notmatch '^TEST_EXIT=' } | Out-Host
    if (($t | Select-Object -Last 1) -ne 'TEST_EXIT=0') { throw 'Có test hỏng' }
}

if ($Deb) {
    $d = Ub-Ssh "cd ~/$work/src/linux && bash packaging/make-deb.sh ~/$work/target/$profile/conbangtv ~/$work/dist thu"
    $d
    # Không đặt tên biến $deb: PowerShell không phân biệt hoa thường — trùng tham số [switch]$Deb
    $debFile = ($d | Where-Object { $_ -match '^DEB=' } | Select-Object -Last 1) -replace '^DEB=', ''
    if (-not $debFile) { throw 'Đóng gói .deb lỗi' }
    $dist = Join-Path $repo 'dist\linux'
    New-Item -ItemType Directory -Force $dist | Out-Null
    Ub-Get $debFile (Join-Path $dist (Split-Path $debFile -Leaf)) | Out-Null
    "Tệp: dist\linux\$(Split-Path $debFile -Leaf)"
}

if ($Run) {
    Ub-Ssh "pkill -x conbangtv; sleep 0.5; systemd-run --user --collect --quiet --unit=conbangtv-thu-`$RANDOM -E CONBANG_DEBUG=1 -E RUST_BACKTRACE=1 ~/$work/target/$profile/conbangtv && echo 'Đã mở app trên màn hình máy Ubuntu'"
}
