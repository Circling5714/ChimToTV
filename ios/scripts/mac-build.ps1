# Dựng IPA / bản Mac trên Mac mini trong nhà — thay GitHub Actions cho bản thử nhanh: không phải commit / đẩy nhánh ios.
#   .\ios\scripts\mac-build.ps1             test lõi + IPA bản thử (Beta, có cổng gỡ lỗi)
#   .\ios\scripts\mac-build.ps1 -Release    thêm bản chính (Release)
#   .\ios\scripts\mac-build.ps1 -Mac        bản Mac (Mac Catalyst) thay cho IPA: bản thử chỉ dựng cho Intel (nhanh), bản chính
#                                           (-Release) dựng Intel + Apple Silicon
#   .\ios\scripts\mac-build.ps1 -Mac -Run   … rồi chép bản Mac thử vào ~/ConBangTV/apps trên Mac và mở (cổng gỡ lỗi 8790)
#   .\ios\scripts\mac-build.ps1 -NoTest     bỏ test lõi
#   .\ios\scripts\mac-build.ps1 -SyncOnly   chỉ gửi mã nguồn + dựng project (xcodegen), không biên dịch
# Mã nguồn lấy từ thư mục làm việc của máy này (cả phần chưa commit): ios\ và app\src\main\assets\ (trình phát dùng chung).
# Dựng và đóng gói giống .github/workflows/ios.yml (cùng ios/scripts/package.sh / package-mac.sh). Tệp về dist\mac\ (dist\ không
# lên git). Bản phát hành chính thức vẫn dựng trên GitHub: Xcode mới hơn, máy sạch.
#
# Mac: Sonoma 14.4 → tối đa Xcode 15.4 (Swift 5.10, SDK iOS 17.5), cũ hơn Xcode 16.x của GitHub — mã phải dựng được trên cả hai.
# Đăng nhập: tệp $env:CONBANG_MAC_INFO, mặc định ..\_May_Test_App\info.txt (ngoài repo, danh sách các máy thử): dòng tên máy có
# chữ «Mac», rồi dòng địa-chỉ/tài-khoản/mật-khẩu.
# Mật khẩu chỉ nằm trong biến môi trường của tiến trình này; mã NextDNS (keys\nextdns.txt) đi qua stdin của ssh, không nằm trên
# dòng lệnh. Trên Mac: ~/ConBangTV/ChimToTV (mã nguồn, xoá và chép lại mỗi lần — tên thư mục không được là «src»: xcodegen ghép nhầm nhóm ../app/src/main/assets), ~/ConBangTV/dd (DerivedData giữ lại → lần sau chỉ
# dựng lại phần đổi), ~/ConBangTV/cai-xcode.log (nhật ký cài Xcode).
param([switch]$Release, [switch]$NoTest, [switch]$SyncOnly, [switch]$Mac, [switch]$Run)
$ErrorActionPreference = "Stop"
$OutputEncoding = New-Object Text.UTF8Encoding $false
[Console]::OutputEncoding = New-Object Text.UTF8Encoding $false
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $root

$info = if ($env:CONBANG_MAC_INFO) { $env:CONBANG_MAC_INFO } else { Join-Path (Split-Path $root) "_May_Test_App\info.txt" }
if (-not (Test-Path $info)) { throw "Không thấy tệp đăng nhập Mac: $info (để chỗ khác thì đặt biến CONBANG_MAC_INFO)" }
# Tệp có thể chứa nhiều máy: tìm dòng tên có chữ «Mac», lấy dòng địa-chỉ/tài-khoản/mật-khẩu đầu tiên ngay sau nó
$lines = @(Get-Content $info -Encoding UTF8)
$label = @(0..($lines.Count - 1) | Where-Object { $lines[$_] -match '(?i)\bmac' -and $lines[$_] -notmatch '/' })[0]
if ($null -eq $label) { throw "Trong $info không thấy dòng tên máy có chữ «Mac»" }
$cred = @($lines[($label + 1)..($lines.Count - 1)] | Where-Object { $_ -match '^\s*[^\s/]+/[^/]+/.' })[0]
if (-not $cred) { throw "Sau dòng «$($lines[$label].Trim())» trong $info không có dòng địa-chỉ/tài-khoản/mật-khẩu" }
$f = $cred -split '/'
$macHost = $f[0].Trim(); $macUser = $f[1].Trim()
$env:CONBANG_MAC_PW = ($f[2..($f.Count - 1)] -join '/').Trim()

$nextDns = if (Test-Path keys\nextdns.txt) { (Get-Content keys\nextdns.txt -Raw).Trim() } else { "" }
if ($nextDns -notmatch '^[a-z0-9]*$') { throw "keys\nextdns.txt: mã NextDNS chỉ gồm chữ thường và số" }
if (-not $nextDns) { Write-Warning "Không có keys\nextdns.txt — bản dựng bỏ NextDNS khỏi chuỗi DNS mã hoá" }

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("conbang-mac-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory $tmp | Out-Null
# SSH_ASKPASS: in mật khẩu từ biến môi trường (qua PowerShell — "echo %VAR%" của cmd làm hỏng ký tự đặc biệt)
$ask = Join-Path $tmp "askpass.cmd"
[IO.File]::WriteAllText($ask, "@powershell -NoProfile -NonInteractive -Command `"[Console]::Out.WriteLine(`$env:CONBANG_MAC_PW)`"`r`n", [Text.Encoding]::ASCII)
# Giữ giá trị cũ để trả lại lúc xong (chạy trong một phiên PowerShell đang dùng SSH_ASKPASS cho việc khác)
$oldEnv = @{}
foreach ($k in "SSH_ASKPASS", "SSH_ASKPASS_REQUIRE", "DISPLAY") { $oldEnv[$k] = [Environment]::GetEnvironmentVariable($k) }
$env:SSH_ASKPASS = $ask; $env:SSH_ASKPASS_REQUIRE = "force"; $env:DISPLAY = "none"
$opts = @("-o", "StrictHostKeyChecking=no", "-o", "UserKnownHostsFile=NUL", "-o", "LogLevel=ERROR", "-o", "ConnectTimeout=10",
          "-o", "PreferredAuthentications=password,keyboard-interactive", "-o", "PubkeyAuthentication=no")
$started = Get-Date

try {
    # ---- 1. Gói mã nguồn: tệp git theo dõi + tệp mới chưa bị .gitignore loại (bỏ tệp đã xoá trong máy) ----
    $files = @(git ls-files -co --exclude-standard -- ios app/src/main/assets | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
    if ($files.Count -lt 10) { throw "Danh sách tệp mã nguồn lạ ($($files.Count) tệp)" }
    $listFile = Join-Path $tmp "files.txt"
    [IO.File]::WriteAllLines($listFile, $files, (New-Object Text.UTF8Encoding $false))
    $tar = Join-Path $tmp "src.tar"
    & "$env:SystemRoot\System32\tar.exe" -cf $tar -T $listFile
    if ($LASTEXITCODE) { throw "Đóng gói mã nguồn lỗi" }
    Write-Host ("Mã nguồn: {0} tệp, {1:N0} KB → {2}@{3}" -f $files.Count, ((Get-Item $tar).Length / 1KB), $macUser, $macHost)
    & scp.exe @opts $tar "${macUser}@${macHost}:ConBangTV/src.tar"
    if ($LASTEXITCODE) { throw "Không gửi được mã nguồn sang Mac (scp mã $LASTEXITCODE)" }

    # ---- 2. Dựng trên Mac (kịch bản đi qua stdin, mã NextDNS nằm trong đó) ----
    $flags = "SYNCONLY=$([int][bool]$SyncOnly); TEST=$([int](-not $NoTest)); RELEASE=$([int][bool]$Release); MAC=$([int][bool]$Mac); " +
             "RUN=$([int][bool]$Run); BUILD='$(Get-Date -Format 'MMdd.HHmm')'; NEXTDNS_ID='$nextDns'"
    $script = $flags + "`n" + @'
set -euo pipefail
export PATH=/usr/local/bin:$PATH
B="$HOME/ConBangTV"
rm -rf "$B/ChimToTV"; mkdir -p "$B/ChimToTV"; tar -xf "$B/src.tar" -C "$B/ChimToTV"; rm -f "$B/src.tar"
cd "$B/ChimToTV/ios"
xcodegen generate --quiet
echo "xcodegen: đã dựng ConBangTV.xcodeproj"
[ "$SYNCONLY" = 1 ] && exit 0
if ! xcode-select -p | grep -q '^/Applications/Xcode'; then
  echo "Mac chưa có Xcode — xem ~/ConBangTV/cai-xcode.log"; exit 3
fi
xcodebuild -version | head -1
beautify() { xcbeautify --quiet --disable-colored-output; }
if [ "$TEST" = 1 ]; then
  echo "== Test lõi (macOS)"
  xcodebuild test -project ConBangTV.xcodeproj -scheme CoreTests -destination 'platform=macOS' -derivedDataPath "$B/dd-test" \
    CODE_SIGN_IDENTITY=- CODE_SIGNING_REQUIRED=NO | beautify
fi
nosign="CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY="
if [ "$MAC" = 1 ]; then
  # Bản Mac (Mac Catalyst). package-mac.sh đóng gói mọi cấu hình có .app — bỏ bản của lần trước để không đóng gói nhầm
  rm -rf "$B/dd/Build/Products/Release-maccatalyst" "$B/dd/Build/Products/Beta-maccatalyst/ConBangTV.app"
  dest='generic/platform=macOS,variant=Mac Catalyst'
  echo "== Bản Mac thử (Beta, Intel), build $BUILD"
  xcodebuild build -project ConBangTV.xcodeproj -scheme ConBangTV -configuration Beta -destination "$dest" \
    -derivedDataPath "$B/dd" ARCHS=x86_64 ONLY_ACTIVE_ARCH=NO CURRENT_PROJECT_VERSION="$BUILD" NEXTDNS_ID="$NEXTDNS_ID" \
    $nosign | beautify
  if [ "$RELEASE" = 1 ]; then
    echo "== Bản Mac chính (Release, Intel + Apple Silicon)"
    xcodebuild build -project ConBangTV.xcodeproj -scheme ConBangTV -configuration Release -destination "$dest" \
      -derivedDataPath "$B/dd" ARCHS="x86_64 arm64" ONLY_ACTIVE_ARCH=NO NEXTDNS_ID="$NEXTDNS_ID" $nosign | beautify
  fi
  ln -sfn "$B/dd" build
  bash scripts/package-mac.sh
  if [ "$RUN" = 1 ]; then
    # Bản thử chạy từ ~/ConBangTV/apps (không đụng /Applications); tắt bản thử cũ trước
    pkill -f "$B/apps/ConBangTV.app/Contents/MacOS/ConBangTV" 2>/dev/null && sleep 1 || true
    rm -rf "$B/apps/ConBangTV.app"; mkdir -p "$B/apps"
    ditto "build/Build/Products/Beta-maccatalyst/ConBangTV.app" "$B/apps/ConBangTV.app"
    open "$B/apps/ConBangTV.app" && echo "Đã mở bản Mac thử: $B/apps/ConBangTV.app"
  fi
  exit 0
fi
# package.sh đóng gói mọi cấu hình có .app — bỏ bản chính của lần trước để không đóng gói nhầm
rm -rf "$B/dd/Build/Products/Release-iphoneos" "$B/dd/Build/Products/Beta-iphoneos/ConBangTV.app"
echo "== Bản thử (Beta), build $BUILD"
xcodebuild build -project ConBangTV.xcodeproj -scheme ConBangTV -configuration Beta -sdk iphoneos \
  -destination 'generic/platform=iOS' -derivedDataPath "$B/dd" CURRENT_PROJECT_VERSION="$BUILD" \
  NEXTDNS_ID="$NEXTDNS_ID" CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY= | beautify
if [ "$RELEASE" = 1 ]; then
  echo "== Bản chính (Release)"
  xcodebuild build -project ConBangTV.xcodeproj -scheme ConBangTV -configuration Release -sdk iphoneos \
    -destination 'generic/platform=iOS' -derivedDataPath "$B/dd" NEXTDNS_ID="$NEXTDNS_ID" \
    CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY= | beautify
fi
ln -sfn "$B/dd" build
bash scripts/package.sh
'@
    $script.Replace("`r`n", "`n") | & ssh.exe @opts "$macUser@$macHost" "bash -s"
    $rc = $LASTEXITCODE
    if ($rc -eq 3) { throw "Mac chưa có Xcode (nhật ký cài: ~/ConBangTV/cai-xcode.log trên Mac)" }
    if ($rc) { throw "Dựng trên Mac lỗi (mã $rc) — xem các dòng ở trên" }
    if ($SyncOnly) { Write-Host "Đã gửi mã nguồn + dựng project trên Mac." -ForegroundColor Green; return }

    # ---- 3. Kéo IPA / .zip bản Mac về dist\mac\ ----
    $out = Join-Path $root "dist\mac"
    New-Item -ItemType Directory -Force $out | Out-Null
    $ext = if ($Mac) { "zip" } else { "ipa" }
    & scp.exe @opts "${macUser}@${macHost}:ConBangTV/ChimToTV/ios/dist/*.$ext" $out
    if ($LASTEXITCODE) { throw "Không lấy được tệp .$ext về" }
    Get-ChildItem $out -Filter "*.$ext" | Where-Object { $_.LastWriteTime -ge $started.AddMinutes(-1) } | ForEach-Object {
        Write-Host ("{0}  {1:N0} byte  SHA-256 {2}" -f $_.FullName, $_.Length, (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower())
    }
    Write-Host ("Xong sau {0:mm\:ss}" -f ((Get-Date) - $started)) -ForegroundColor Green
} finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item Env:CONBANG_MAC_PW -ErrorAction SilentlyContinue
    foreach ($k in $oldEnv.Keys) { [Environment]::SetEnvironmentVariable($k, $oldEnv[$k]) }
}
