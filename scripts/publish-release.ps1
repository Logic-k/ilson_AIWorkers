# 일손(ilson) 무설치 자동 업데이트 배포 스크립트 (GitHub Releases + latest.json)
$ErrorActionPreference = "Stop"

$repo = "Logic-k/ilson_AIWorkers"
$conf = Get-Content "src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json
$version = $conf.version
$tag = "v$version"

Write-Host "=== 일손 $tag 무설치 업데이트 배포 준비 ===" -ForegroundColor Cyan

# 1. 실행 중인 ilson.exe를 끄지 않고 이름만 변경해 빌드 잠금 방지
if (Test-Path "src-tauri\target\release\ilson.exe") {
    $stamp = Get-Date -Format "HHmmss"
    Move-Item -Force "src-tauri\target\release\ilson.exe" "src-tauri\target\release\ilson_running_$stamp.exe" -ErrorAction SilentlyContinue
}

# 2. 서명 키 환경 변수 설정 및 Tauri 빌드
$keyPath = "$env:USERPROFILE\.tauri\ilson.key"
if (!(Test-Path $keyPath)) {
    throw "서명 비밀키를 찾을 수 없습니다: $keyPath"
}
$env:TAURI_SIGNING_PRIVATE_KEY = $keyPath
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""

$setupFile = "src-tauri\target\release\bundle\nsis\일손_${version}_x64-setup.exe"
$sigFile = "$setupFile.sig"

if (!(Test-Path $setupFile) -or !(Test-Path $sigFile)) {
    npx tauri build
}

# 3. GitHub Release 업로드용 영문 파일명 복사 (URL 인코딩 문제 방지)
$asciiSetup = "src-tauri\target\release\bundle\nsis\ilson_${version}_x64-setup.exe"
Copy-Item -Force $setupFile $asciiSetup

$signature = (Get-Content $sigFile -Raw).Trim()
$pubDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$downloadUrl = "https://github.com/$repo/releases/download/$tag/ilson_${version}_x64-setup.exe"

$latest = @{
    version = $version
    notes = "일손 $tag 자동 업데이트"
    pub_date = $pubDate
    platforms = @{
        "windows-x86_64" = @{
            signature = $signature
            url = $downloadUrl
        }
    }
}
$latestPath = "src-tauri\target\release\bundle\nsis\latest.json"
$latest | ConvertTo-Json -Depth 5 | Set-Content -Path $latestPath -Encoding UTF8

Write-Host "latest.json 생성 완료: $latestPath" -ForegroundColor Green

# 4. GitHub Release 생성 및 업로드
gh release view $tag --repo $repo 2>$null | Out-Null
if ($LASTEXITCODE -eq 0) {
    gh release upload $tag $asciiSetup $setupFile $latestPath --clobber --repo $repo
} else {
    gh release create $tag $asciiSetup $setupFile $latestPath --title "일손 $tag" --notes "일손 $tag 무설치 자동 업데이트 배포판" --repo $repo
}

Write-Host "=== GitHub Release ($tag) 업로드 완료! ===" -ForegroundColor Green
