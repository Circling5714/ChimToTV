# Đăng một bản ĐÃ DỰNG lên GitHub Releases. Cờ "Latest" đặt sẵn theo nền tảng — không phải nhớ tay:
#
#   tv       tag v<bản>       ConBangTV-<bản>.apk + ConBangTV.apk + ChimToTV.apk (hai bản sao giống hệt,   LUÔN là Latest
#                             tên cố định)
#   windows  tag win-v<bản>   ConBangTV-Windows-<bản>.exe                                         KHÔNG BAO GIỜ là Latest
#   ipad     tag ios-v<bản>   ConBangTV-iPad-<bản>.ipa                                            KHÔNG BAO GIỜ là Latest
#
# Vì sao phải đúng — hai thứ đi theo bản đang là Latest:
#   1. app TV tự cập nhật: Updater.kt hỏi .../releases/latest rồi lấy tệp .apk đầu tiên (tên nào cũng được — từ bản 1.3);
#   2. link tải cố định cho app Downloader trên Android TV (chỉ tải đúng một địa chỉ, không đọc được API):
#        https://github.com/Circling5714/ChimToTV/releases/latest/download/ConBangTV.apk      (tên mới, từ 13/11/2026)
#        https://github.com/Circling5714/ChimToTV/releases/latest/download/ChimToTV.apk       (tên cũ — vẫn phải có)
#      Hai link này cần tệp tên ConBangTV.apk / ChimToTV.apk có trong MỌI bản TV — nên bản TV đính kèm ba tệp. ĐỪNG bỏ tệp tên
#      cũ: mã Downloader 9192365 (aftv.news) trỏ vào link cũ, và màn «Cài lên tivi» của TV 2.16–2.17 / iPad 1.16–1.17 /
#      Windows 1.14–1.15 đã cài in link cũ. Ghi chú của bản TV được thêm mục "## Cài mới trên Android TV" ghi mã Downloader.
# Bản Windows / iPad mà thành Latest thì mọi TV hết thấy bản mới và hai link trên trả 404.
#
# Link tải cố định của bản Windows / iPad (luôn là bản mới nhất, như link APK ở trên) — mỗi nền tảng một release "cuốn chiếu"
# với tag không bao giờ đổi và tệp tên cố định; mỗi lần đăng bản mới script chép đè tệp vào đó:
#   https://github.com/Circling5714/ChimToTV/releases/download/conbangtv_windows/ConBangTV-Windows.exe
#   https://github.com/Circling5714/ChimToTV/releases/download/conbangtv_ios/ConBangTV-iPad.ipa
# ĐỪNG đổi hai tag / hai tên tệp này: link đã gửi cho người dùng và nằm trên trang giới thiệu. Ba bộ tự cập nhật không nhìn
# thấy hai release này (TV đọc releases/latest; Windows / iPad chỉ nhận tag win-v… / ios-v…).
#
# Cách dùng (sau khi đã dựng tệp và đã git push):
#   .\release.ps1 tv      -Notes ghi-chu.md
#   .\release.ps1 windows -Notes ghi-chu.md
#   .\release.ps1 ipad    -Notes ghi-chu.md [-File đường\dẫn\ConBangTV-iPad-<bản>.ipa]
#   Thêm -DryRun: kiểm tệp, in lệnh sẽ chạy, không đăng gì.
#   .\release.ps1 tv -Verify      (windows / ipad cũng vậy): chỉ kiểm lại bản đã đăng, không đăng, không sửa gì.
#   .\release.ps1 windows -FixedOnly   (ipad cũng vậy): không đăng bản mới — chỉ chép tệp của bản ĐÃ đăng vào link cố định
#                                      (lần đầu tạo link, hoặc sửa khi link lệch).
# Số bản đọc từ app\build.gradle.kts (tv), windows\ConBangTV\ConBangTV.csproj (windows), ios\project.yml (ipad).
#
# Kiểm sau khi đăng (và khi -Verify): Latest là bản TV — lúc đăng mà lệch thì script tự trả về bản TV mới nhất; tệp trên
# GitHub khớp SHA-256 với tệp trong máy; bản TV: tải thử qua hai link cố định (tên mới, tên cũ) và so SHA-256; bản Windows /
# iPad mới nhất: tệp ở link cố định khớp SHA-256 và link tải được.
param(
    [Parameter(Mandatory = $true, Position = 0)][ValidateSet("tv", "windows", "ipad")][string]$Platform,
    [string]$Notes = "",
    [string]$File = "",
    [switch]$DryRun,
    [switch]$Verify,
    [switch]$FixedOnly
)
$ErrorActionPreference = "Stop"

# Đường dẫn người gọi đưa vào tính theo thư mục họ đang đứng — đổi sang đường dẫn đầy đủ trước khi chuyển thư mục
if (-not $Verify -and -not $FixedOnly) {
    if (-not $Notes) { throw "Thiếu -Notes <tệp ghi chú phát hành>" }
    if (-not (Test-Path $Notes)) { throw "Không thấy tệp ghi chú: $Notes" }
    $Notes = (Resolve-Path $Notes).Path
}
if ($File) {
    if (-not (Test-Path $File)) { throw "Không thấy tệp: $File" }
    $File = (Resolve-Path $File).Path
}
Set-Location $PSScriptRoot

$repo = "Circling5714/ChimToTV"
# Tên cố định của APK bản TV: tên mới (link chính) và tên cũ (giữ cho mã Downloader + app đã cài — xem đầu tệp)
$fixedName = "ConBangTV.apk"
$legacyName = "ChimToTV.apk"
$fixedLink = "https://github.com/$repo/releases/latest/download/$fixedName"
$legacyLink = "https://github.com/$repo/releases/latest/download/$legacyName"
# Mã của app Downloader (aftv.news, trỏ vào $legacyLink — tạo trước khi đổi tên tệp) và trang hướng dẫn — cùng giá trị với
# Invite.kt (TV), Invite.swift (iPad), Invite.cs (Windows) và README
$downloaderCode = "9192365"
$guideUrl = "https://github.com/$repo/blob/main/README.md#downloader"
$certSha256 = "949ef2b52f3cf895d7fe5ff9db495f273e6fd35d02b23b4a521df43957a0177b"
# Release "cuốn chiếu" giữ link tải cố định của bản Windows / iPad (xem đầu tệp). Prefix: tag của các bản có số.
$rolling = @{
    windows = @{
        Tag = "conbangtv_windows"; Name = "ConBangTV-Windows.exe"; Prefix = "win-v"; What = "Windows"
        Title = "Côn Bằng TV cho Windows — link cố định (luôn là bản mới nhất)"
        How = "Tải về, để vào thư mục nào cũng được rồi mở (Windows 10 / 11). Đã cài rồi thì không cần tải lại: app tự cập nhật."
    }
    ipad = @{
        Tag = "conbangtv_ios"; Name = "ConBangTV-iPad.ipa"; Prefix = "ios-v"; What = "iPad / iPhone"
        Title = "Côn Bằng TV cho iPad / iPhone — link cố định (luôn là bản mới nhất)"
        How = "Cài bằng TrollStore (iOS / iPadOS 15 trở lên): mở tệp bằng TrollStore rồi chọn Install. Đã cài rồi thì không cần tải lại: app tự cập nhật."
    }
}
function RollingLink($r) { "https://github.com/$repo/releases/download/$($r.Tag)/$($r.Name)" }

function Sha256([string]$path) { (Get-FileHash $path -Algorithm SHA256).Hash.ToLower() }

# Chạy gh, lỗi thì dừng. Gọi thẳng gh.exe: tên lệnh trong PowerShell không phân biệt hoa / thường, hàm tên "gh" sẽ tự gọi
# lại chính nó.
function Gh {
    $out = & gh.exe @args
    if ($LASTEXITCODE -ne 0) { throw "Lệnh gh $($args -join ' ') lỗi (mã $LASTEXITCODE)" }
    $out
}

# Việc chỉ quan trọng lúc đăng thật: chạy thử (-DryRun) thì chỉ cảnh báo
function Stop-OrWarn([string]$message) {
    if ($DryRun) { Write-Warning $message } else { throw $message }
}

function TagVersion([string]$tag) {
    try { [version]($tag -replace '^[a-z-]*v', '') } catch { [version]"0.0" }
}

function LatestTag { ("" + (Gh api "repos/$repo/releases/latest" --jq ".tag_name")).Trim() }

# ---- Nền tảng → tag, tiêu đề, tệp, cờ Latest ----
switch ($Platform) {
    "tv" {
        $ver = (Select-String -Path app\build.gradle.kts -Pattern 'versionName = "(.+)"').Matches[0].Groups[1].Value
        $tag = "v$ver"; $title = "Côn Bằng TV $ver"; $latest = $true; $name = "ConBangTV-$ver.apk"
    }
    "windows" {
        $full = ([xml](Get-Content windows\ConBangTV\ConBangTV.csproj)).Project.PropertyGroup.Version | Where-Object { $_ } | Select-Object -First 1
        $ver = $full -replace '\.0$', ''
        $tag = "win-v$ver"; $title = "Côn Bằng TV cho Windows $ver"; $latest = $false; $name = "ConBangTV-Windows-$ver.exe"
    }
    "ipad" {
        $ver = (Select-String -Path ios\project.yml -Pattern 'MARKETING_VERSION:\s*"?([0-9.]+)"?').Matches[0].Groups[1].Value
        $tag = "ios-v$ver"; $title = "Côn Bằng TV cho iPad $ver"; $latest = $false; $name = "ConBangTV-iPad-$ver.ipa"
    }
}
if (-not $File) {
    $File = Join-Path $PSScriptRoot "dist\$name"
    if (-not (Test-Path $File)) { throw "Không thấy $File — dựng bản $ver trước (tv: .\build.ps1 · windows: .\windows\build.ps1 · ipad: tải artifact rồi dùng -File)" }
}
if ((Split-Path $File -Leaf) -ne $name) { throw "Tệp phải tên $name (bản $ver đọc từ mã nguồn), đang là $(Split-Path $File -Leaf)" }
# Tên các tệp của bản này trên GitHub — bản TV có thêm hai bản sao tên cố định (tên mới, tên cũ)
$assetNames = @($name)
if ($Platform -eq "tv") { $assetNames += $fixedName, $legacyName }

$tags = @(Gh api "repos/$repo/releases?per_page=100" --jq '.[].tag_name')
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("chimto-release-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force $tmp | Out-Null

# Kiểm một bản đã đăng. $heal: Latest lệch khỏi bản TV thì sửa luôn (lúc đăng); không thì chỉ báo lỗi (-Verify).
function Test-Published([bool]$heal) {
    # 1. Latest phải là một bản TV; bản TV mới nhất phải chính là Latest
    $now = LatestTag
    $want = @($tags | Where-Object { $_ -match '^v[0-9]' } | Sort-Object { TagVersion $_ } -Descending)[0]
    if ($now -ne $want) {
        if (-not $heal) { throw "Latest đang là $now, cần là $want (bản TV mới nhất) — sửa: gh release edit $want --latest" }
        Write-Warning "Latest đang là $now, cần là $want (bản TV mới nhất) — đang trả về"
        Gh release edit $want --repo $repo --latest | Out-Null
        $now = LatestTag
        if ($now -ne $want) { throw "Latest vẫn là $now, cần là $want — sửa tay: gh release edit $want --latest" }
    }
    Write-Host "Latest: $now"

    # 2. Tệp trên GitHub khớp tệp trong máy
    $local = Sha256 $File
    $rel = ((Gh api "repos/$repo/releases/tags/$tag") -join "`n") | ConvertFrom-Json
    foreach ($leaf in $assetNames) {
        $up = $rel.assets | Where-Object { $_.name -eq $leaf }
        if (-not $up) { throw "Release $tag thiếu tệp $leaf" }
        if ($up.digest -ne "sha256:$local") { throw "Tệp $leaf trên GitHub không khớp SHA-256 với $File" }
        Write-Host ("{0}: {1:N0} byte, {2}" -f $leaf, $up.size, $up.digest)
    }

    # 3. Bản TV đang là Latest: hai link cố định (tên mới; tên cũ của mã Downloader) phải tải về đúng tệp này
    if ($Platform -eq "tv" -and $now -eq $tag) {
        foreach ($link in $fixedLink, $legacyLink) {
            $got = Join-Path $tmp "tai-ve.apk"
            $ok = $false
            foreach ($wait in 0, 5, 15) {
                Start-Sleep -Seconds $wait
                & curl.exe -sL -m 300 -o $got $link
                if ($LASTEXITCODE -eq 0 -and (Test-Path $got) -and (Sha256 $got) -eq $local) { $ok = $true; break }
            }
            if (-not $ok) { throw "Link cố định chưa trả đúng tệp bản ${ver}: $link" }
            Write-Host "Link cố định tải về đúng bản ${ver}: $link"
        }
    }

    # 4. Bản Windows / iPad mới nhất: tệp ở link cố định phải chính là tệp này, và link phải tải được
    $r = $rolling[$Platform]
    if ($r) {
        $newest = @($tags | Where-Object { $_.StartsWith($r.Prefix) } | Sort-Object { TagVersion $_ } -Descending)[0]
        if ($tag -ne $newest) {
            Write-Host "(bản $tag không phải bản $($r.What) mới nhất — link cố định đi theo $newest, không kiểm ở đây)"
        } else {
            $link = RollingLink $r
            if ($tags -notcontains $r.Tag) { throw "Chưa có link cố định cho bản $($r.What) — tạo: .\release.ps1 $Platform -FixedOnly" }
            $roll = ((Gh api "repos/$repo/releases/tags/$($r.Tag)") -join "`n") | ConvertFrom-Json
            $up = $roll.assets | Where-Object { $_.name -eq $r.Name }
            if (-not $up) { throw "Release $($r.Tag) thiếu tệp $($r.Name) — sửa: .\release.ps1 $Platform -FixedOnly" }
            if ($up.digest -ne "sha256:$local") { throw "Link cố định chưa phải bản ${ver} (SHA-256 khác) — sửa: .\release.ps1 $Platform -FixedOnly" }
            # Tệp nhỏ thì tải hẳn về so SHA-256; tệp lớn (.exe ~70 MB) chỉ hỏi đầu trang: phải 200 và đúng số byte
            $ok = $false
            foreach ($wait in 0, 5, 15) {
                Start-Sleep -Seconds $wait
                if ($up.size -lt 20MB) {
                    $got = Join-Path $tmp "tai-ve-co-dinh"
                    & curl.exe -sL -m 300 -o $got $link
                    if ($LASTEXITCODE -eq 0 -and (Test-Path $got) -and (Sha256 $got) -eq $local) { $ok = $true; break }
                } else {
                    $headers = @(& curl.exe -sIL -m 60 $link)
                    $status = @($headers | Where-Object { $_ -match '^HTTP/' })[-1]
                    $length = @($headers | Where-Object { $_ -match '^content-length:\s*(\d+)' } | ForEach-Object { [long]($_ -replace '\D', '') })[-1]
                    if ($status -match ' 200' -and $length -eq $up.size) { $ok = $true; break }
                }
            }
            if (-not $ok) { throw "Link cố định chưa tải được đúng tệp bản ${ver}: $link" }
            Write-Host "Link cố định là bản ${ver}: $link"
        }
    }
}

# Chép tệp của bản này vào release "cuốn chiếu" giữ link cố định của nền tảng (tạo release nếu chưa có).
function Update-Rolling {
    $r = $rolling[$Platform]
    if (-not $r) { return }
    $link = RollingLink $r
    $local = Sha256 $File
    $body = "Tệp ở đây **luôn là bản $($r.What) mới nhất** — link tải không bao giờ đổi:`n`n$link`n`n" +
        "Hiện là **bản $ver** (giống hệt tệp của release [$tag](https://github.com/$repo/releases/tag/$tag)) · SHA-256 ``$local```n`n$($r.How)`n"
    $bodyFile = Join-Path $tmp "link-co-dinh.md"
    [IO.File]::WriteAllText($bodyFile, $body, (New-Object Text.UTF8Encoding $false))
    if ($tags -notcontains $r.Tag) {
        Write-Host "Tạo release giữ link cố định: $($r.Tag)"
        Gh release create $r.Tag --repo $repo --title $r.Title --notes-file $bodyFile --latest=false --target main | Out-Host
        $script:tags = @($r.Tag) + $tags
    }
    $roll = ((Gh api "repos/$repo/releases/tags/$($r.Tag)") -join "`n") | ConvertFrom-Json
    $old = $roll.assets | Where-Object { $_.name -eq $r.Name }
    if ($old -and $old.digest -eq "sha256:$local") {
        Write-Host "Link cố định đã là bản ${ver}"
    } else {
        # Tải lên dưới tên tạm rồi mới đổi tên: link cố định chỉ gián đoạn chừng một giây (xoá tệp cũ → đổi tên tệp mới), không
        # phải suốt lúc tải tệp lên (tệp .exe ~70 MB)
        $tmpName = $r.Name + ".moi"
        $copy = Join-Path $tmp $tmpName
        Copy-Item $File $copy
        Gh release upload $r.Tag $copy --repo $repo --clobber | Out-Host
        $roll = ((Gh api "repos/$repo/releases/tags/$($r.Tag)") -join "`n") | ConvertFrom-Json
        $new = $roll.assets | Where-Object { $_.name -eq $tmpName }
        if (-not $new -or $new.digest -ne "sha256:$local") { throw "Tệp tải lên $($r.Tag) không khớp SHA-256 với $File" }
        if ($old) { Gh api -X DELETE "repos/$repo/releases/assets/$($old.id)" | Out-Null }
        Gh api -X PATCH "repos/$repo/releases/assets/$($new.id)" -f "name=$($r.Name)" | Out-Null
        Write-Host "Link cố định → bản ${ver}: $link"
    }
    # Ghi chú của release cuốn chiếu luôn nói đang là bản nào. --latest=false: sửa release không được làm nó thành Latest.
    Gh release edit $r.Tag --repo $repo --title $r.Title --notes-file $bodyFile --latest=false | Out-Null
}

try {
    if ($Verify) {
        if ($tags -notcontains $tag) { throw "Chưa có release $tag trên GitHub" }
        Test-Published $false
        Write-Host "Đạt: $tag" -ForegroundColor Green
        return
    }

    # ---- Chỉ chép tệp của bản đã đăng vào link cố định (lần đầu tạo link / sửa link lệch) ----
    if ($FixedOnly) {
        $r = $rolling[$Platform]
        if (-not $r) { throw "Bản TV không cần -FixedOnly: hai link cố định của bản TV đi theo Latest ($fixedLink, $legacyLink)" }
        if ($tags -notcontains $tag) { throw "Chưa có release $tag trên GitHub — đăng bản $ver trước" }
        $pub = ((Gh api "repos/$repo/releases/tags/$tag") -join "`n") | ConvertFrom-Json
        $up = $pub.assets | Where-Object { $_.name -eq $name }
        if (-not $up -or $up.digest -ne "sha256:$(Sha256 $File)") { throw "Tệp $File không khớp SHA-256 với tệp $name của release $tag" }
        Write-Host ("Link cố định: {0} ← {1} (bản {2}, {3:N0} byte)" -f (RollingLink $r), $name, $ver, (Get-Item $File).Length)
        if ($DryRun) { Write-Host "(DryRun) không sửa gì."; return }
        Update-Rolling
        Test-Published $true
        Write-Host "Xong: link cố định của bản $($r.What) là bản $ver" -ForegroundColor Green
        return
    }

    if (-not (Select-String -Path $Notes -Pattern '^\s*##\s*Có gì mới' -Encoding UTF8 -Quiet)) {
        throw "Ghi chú phải có mục '## Có gì mới' — app hiện mục này trong hộp thoại cập nhật"
    }

    # ---- Bản TV: đúng gói, đúng số bản, đúng khoá ký (sai khoá là mọi TV từ chối bản cập nhật) ----
    if ($Platform -eq "tv") {
        $tools = Get-ChildItem "$env:LOCALAPPDATA\Android\Sdk\build-tools" -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1
        if ($tools) {
            if (-not $env:JAVA_HOME) { $env:JAVA_HOME = "$env:USERPROFILE\.jdks\jdk-17.0.20.1+1" }
            $badging = "" + (& "$($tools.FullName)\aapt2.exe" dump badging $File | Select-String -Pattern "^package:" | Select-Object -First 1)
            if ($badging -notmatch "name='vn\.chimto\.tv'" -or $badging -notmatch "versionName='$([regex]::Escape($ver))'") {
                throw "APK không đúng gói vn.chimto.tv / bản ${ver}: $badging"
            }
            $certs = & "$($tools.FullName)\apksigner.bat" verify --print-certs $File
            if ($LASTEXITCODE -ne 0 -or -not ($certs -match $certSha256)) {
                throw "APK không ký bằng khoá của app (cần chứng chỉ SHA-256 $certSha256) — kiểm thư mục keys\"
            }
            Write-Host "APK: $badging"
            Write-Host "Chữ ký: đúng chứng chỉ $($certSha256.Substring(0, 8))…$($certSha256.Substring(60))"
        } else {
            Write-Warning "Không thấy Android build-tools — chưa kiểm được chữ ký / số bản của APK (kiểm tay bằng apksigner, aapt2)"
        }
    }

    # ---- Đã đẩy code chưa, tag đã có chưa ----
    & git fetch origin main --quiet
    $head = ("" + (& git rev-parse HEAD)).Trim()
    $remote = ("" + (& git rev-parse origin/main)).Trim()
    if ($head -ne $remote) {
        Stop-OrWarn "HEAD ($($head.Substring(0, 7))) khác origin/main ($($remote.Substring(0, 7))) — đẩy code trước (vd: git push origin <nhánh> <nhánh>:main)"
    }
    if ($tags -contains $tag) { Stop-OrWarn "Release $tag đã có trên GitHub — tăng số bản trước khi đăng" }

    # ---- Tệp đính kèm: bản TV thêm hai bản sao tên cố định ----
    $assets = @($File)
    if ($Platform -eq "tv") {
        foreach ($fixed in $fixedName, $legacyName) {
            $copy = Join-Path $tmp $fixed
            Copy-Item $File $copy
            if ((Sha256 $copy) -ne (Sha256 $File)) { throw "Bản sao $fixed không giống tệp gốc" }
            $assets += $copy
        }
    }
    # Bản TV: thêm cuối ghi chú cách cài mới bằng Downloader (người vào trang Releases thấy ngay mã). Hộp cập nhật trên TV chỉ
    # lấy mục "## Có gì mới" tới tiêu đề "## " kế tiếp, nên mục thêm này không làm hộp dài ra.
    $notesFile = $Notes
    if ($Platform -eq "tv") {
        $text = [IO.File]::ReadAllText($Notes, [Text.Encoding]::UTF8)
        if (-not $text.Contains($downloaderCode)) {
            $text = $text.TrimEnd() + "`n`n## Cài mới trên Android TV`n`nMở app **Downloader** (AFTVnews) trên tivi, nhập mã **$downloaderCode** rồi bấm Go. Hướng dẫn: $guideUrl`n"
            $notesFile = Join-Path $tmp "ghi-chu.md"
            [IO.File]::WriteAllText($notesFile, $text, (New-Object Text.UTF8Encoding $false))
        }
    }
    $flag = if ($latest) { "--latest" } else { "--latest=false" }
    $cmd = @("release", "create", $tag) + $assets + @("--repo", $repo, "--title", $title, "--notes-file", $notesFile, $flag, "--target", "main")
    Write-Host ""
    Write-Host ("gh " + (($cmd | ForEach-Object { if ($_ -match '\s') { "`"$_`"" } else { $_ } }) -join " "))
    Write-Host ("SHA-256 {0}: {1} ({2:N0} byte)" -f $name, (Sha256 $File), (Get-Item $File).Length)
    if ($DryRun) {
        if ($notesFile -ne $Notes) {
            Write-Host "--- ghi chú sẽ đăng (đã thêm mục cài mới) ---"
            Write-Host ([IO.File]::ReadAllText($notesFile, [Text.Encoding]::UTF8))
        }
        if ($rolling[$Platform]) { Write-Host "Sau khi đăng sẽ chép tệp vào link cố định: $(RollingLink $rolling[$Platform])" }
        Write-Host "(DryRun) không đăng gì. Latest hiện giờ: $(LatestTag)"
        return
    }
    Gh @cmd | Out-Host
    $tags = @($tag) + $tags
    Update-Rolling
    Test-Published $true
    Write-Host "Xong: $tag — $title" -ForegroundColor Green
} finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
