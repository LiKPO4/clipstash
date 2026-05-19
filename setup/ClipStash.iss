; Inno Setup 脚本 —— 将 PyInstaller --onedir 输出打包为安装程序
; 用法: ISCC.exe ClipStash.iss

#define MyAppName "ClipStash"
#define MyAppNameCn "需求暂存站"
#define MyAppVersion "1.3.6"
#define MyAppPublisher "LiKPO4"
#define MyAppExeName "ClipStash.exe"
#define MyAppId "LiKPO4.ClipStash"

[Setup]
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
VersionInfoVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={localappdata}\Programs\{#MyAppName}
DisableDirPage=no
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename={#MyAppName}-Setup-v{#MyAppVersion}
SetupIconFile=..\assets\app_icon.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppNameCn}

[Files]
Source: "..\dist\{#MyAppName}\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\{#MyAppName}\_internal\*"; DestDir: "{app}\_internal"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autodesktop}\{#MyAppNameCn}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon
Name: "{autoprograms}\{#MyAppNameCn}"; Filename: "{app}\{#MyAppExeName}"

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; GroupDescription: "快捷方式:"

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "立即启动 {#MyAppNameCn}"; Flags: postinstall skipifsilent nowait

[Registry]
Root: HKCU; Subkey: "Software\{#MyAppName}"; ValueType: string; ValueName: "InstallPath"; ValueData: "{app}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\{#MyAppName}"; ValueType: string; ValueName: "Version"; ValueData: "{#MyAppVersion}"; Flags: uninsdeletekey
