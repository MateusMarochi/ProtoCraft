#define AppName "ProtoCraft"
#ifndef AppVersion
#define AppVersion "0.6.1"
#endif
#define AppPublisher "MateusMarochi"
#define AppURL "https://github.com/MateusMarochi/ProtoCraft"
#define AppExeName "protocraft.exe"
#define AppIconFile "..\assets\protocraft.ico"

[Setup]
AppId={{56B99818-2C25-43C5-997D-27FF9F182734}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
DefaultDirName={autopf}\ProtoCraft
DefaultGroupName=ProtoCraft
UninstallDisplayIcon={app}\{#AppExeName}
SetupIconFile={#AppIconFile}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
OutputDir=..\dist
OutputBaseFilename=protocraft-{#AppVersion}-windows-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
brazilianportuguese.CreateDesktopIcon=Criar um atalho na área de trabalho
brazilianportuguese.AdditionalIcons=Atalhos adicionais:
brazilianportuguese.LaunchProtoCraft=Iniciar o ProtoCraft
english.CreateDesktopIcon=Create a desktop shortcut
english.AdditionalIcons=Additional shortcuts:
english.LaunchProtoCraft=Launch ProtoCraft

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\dist\ProtoCraft\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\ProtoCraft"; Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\{#AppExeName}"
Name: "{autodesktop}\ProtoCraft"; Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProtoCraft}"; Flags: postinstall nowait skipifsilent
