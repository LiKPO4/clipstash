# ClipStash 打包脚本
# 先执行 PyInstaller (--onedir)，再用 Inno Setup 打包为安装程序
# 要求: Python + PyInstaller + Inno Setup 6

param(
    [string]$Version = "1.3.8"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

# 1. 清理旧输出
Write-Host "[1/4] 清理旧输出..." -ForegroundColor Cyan
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "dist\ClipStash"
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "build"

# 2. PyInstaller --onedir（解决 onefile 启动慢问题）
Write-Host "[2/4] PyInstaller 打包 (--onedir)..." -ForegroundColor Cyan
python -m PyInstaller `
    --noconfirm `
    --onedir `
    --windowed `
    --name ClipStash `
    --icon assets\app_icon.ico `
    --add-data "assets;assets" `
    --exclude-module PyQt5 `
    --exclude-module PyQt6 `
    --exclude-module PySide2 `
    --exclude-module PySide6 `
    main.py

if ($LASTEXITCODE -ne 0) {
    Write-Error "PyInstaller 失败"
    exit 1
}

# 3. 更新 .iss 版本号
Write-Host "[3/4] 更新安装脚本版本号..." -ForegroundColor Cyan
$IssPath = Join-Path $PSScriptRoot "ClipStash.iss"
$IssContent = Get-Content $IssPath -Raw
$IssContent = $IssContent -replace '#define MyAppVersion "[\d.]+"', "#define MyAppVersion `"$Version`""
Set-Content $IssPath $IssContent -NoNewline

# 4. Inno Setup 打包
Write-Host "[4/4] Inno Setup 生成安装包..." -ForegroundColor Cyan
$ISCC = $null
$Cmd = Get-Command ISCC.exe -ErrorAction SilentlyContinue
if ($Cmd) {
    $ISCC = $Cmd.Source
}

if (-not $ISCC) {
    $Candidates = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles}\Inno Setup 6\ISCC.exe",
        "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
        "C:\Program Files\Inno Setup 6\ISCC.exe"
    )
    foreach ($Candidate in $Candidates) {
        if (Test-Path $Candidate) {
            $ISCC = (Resolve-Path $Candidate).Path
            break
        }
    }
}

if (-not $ISCC) {
    Write-Error "找不到 ISCC.exe，请安装 Inno Setup 并确保其在 PATH 中"
    exit 1
}

& $ISCC $IssPath
if ($LASTEXITCODE -ne 0) {
    Write-Error "Inno Setup 编译失败"
    exit 1
}

Write-Host "`n打包完成！" -ForegroundColor Green
Write-Host "安装包: dist\ClipStash-Setup-v$Version.exe" -ForegroundColor Yellow
