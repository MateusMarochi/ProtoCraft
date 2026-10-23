#define AppName "ProtoCraft"
#ifndef AppVersion
#define AppVersion "0.6.1"
#endif
#define AppPublisher "MateusMarochi"
#define AppURL "https://github.com/MateusMarochi/ProtoCraft"
#define AppExeName "protocraft.exe"

[Setup]
AppId={{56B99818-2C25-43C5-997D-27FF9F182734}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
DefaultDirName={autopf}\ProtoCraft
DefaultGroupName=ProtoCraft
UninstallDisplayIcon={app}\{#AppExeName}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
OutputDir=..\dist
OutputBaseFilename=ProtoCraft-{#AppVersion}-Setup-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Criar um atalho na área de trabalho"; GroupDescription: "Atalhos adicionais:"; Flags: unchecked

[Files]
Source: "..\dist\ProtoCraft\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\ProtoCraft"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\ProtoCraft"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExeName}"; Description: "Iniciar o ProtoCraft"; Flags: postinstall nowait skipifsilent
