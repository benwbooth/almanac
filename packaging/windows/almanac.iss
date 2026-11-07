; Inno Setup script for the Windows installer.
; Built by .github/workflows/release.yml:
;   iscc /DAppVersion=1.2.3 /DSourceDir=<windeployqt folder> /DOutputDir=<out> almanac.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\dist\Almanac"
#endif
#ifndef OutputDir
  #define OutputDir "..\..\dist"
#endif

[Setup]
AppId={{6B0E2F4A-1C55-4E1B-9D7E-6E1A2B8C3D41}
AppName=Almanac
AppVersion={#AppVersion}
AppVerName=Almanac {#AppVersion}
AppPublisher=Ben Booth
AppPublisherURL=https://github.com/benwbooth/almanac
AppSupportURL=https://github.com/benwbooth/almanac/issues
DefaultDirName={autopf}\Almanac
DefaultGroupName=Almanac
DisableProgramGroupPage=yes
; Per-user install by default (no admin prompt); the person can choose all users.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=Almanac-{#AppVersion}-windows-x64-setup
SetupIconFile=..\icons\almanac.ico
UninstallDisplayIcon={app}\almanac.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Almanac"; Filename: "{app}\almanac.exe"
Name: "{autodesktop}\Almanac"; Filename: "{app}\almanac.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\almanac.exe"; Description: "{cm:LaunchProgram,Almanac}"; Flags: nowait postinstall skipifsilent
