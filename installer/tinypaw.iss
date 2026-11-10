; TinyPaw per-user installer (Inno Setup 6). Build from the repository root:
;   cargo build --release
;   iscc installer\tinypaw.iss
; Output: target\installer\TinyPaw-<version>-setup.exe
;
; Installs to %LOCALAPPDATA%\Programs\TinyPaw without admin rights, adds a
; Start-menu shortcut to the GUI launcher (tinypawd.exe), and optionally puts
; the folder holding the CLI (tinypaw.exe) on the user PATH. Uninstall
; removes the files, shortcut, PATH entry and sign-in Run value, and keeps
; %APPDATA%\TinyPaw unless the user chooses to delete it.

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef BinDir
  #define BinDir "..\target\release"
#endif

[Setup]
AppId={{EBC0CAAE-7D08-4C81-B8B2-66E1F4163166}
AppName=TinyPaw
AppVersion={#AppVersion}
AppPublisher=TinyPaw
AppComments=A paw pointer for pointing and drawing over presentations and game streams.
DefaultDirName={localappdata}\Programs\TinyPaw
DefaultGroupName=TinyPaw
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
ChangesEnvironment=yes
UninstallDisplayIcon={app}\tinypaw.exe
UninstallDisplayName=TinyPaw
OutputDir=..\target\installer
OutputBaseFilename=TinyPaw-{#AppVersion}-setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern

[Tasks]
Name: "addtopath"; Description: "Add the tinypaw command to my PATH"; GroupDescription: "Command line:"

[Files]
Source: "{#BinDir}\tinypaw.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BinDir}\tinypawd.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme

[Icons]
Name: "{userprograms}\TinyPaw"; Filename: "{app}\tinypawd.exe"; WorkingDir: "{app}"; IconFilename: "{app}\tinypaw.exe"; Comment: "Start TinyPaw in the tray"

[Run]
Filename: "{app}\tinypawd.exe"; Description: "Start TinyPaw now"; Flags: postinstall nowait skipifsilent

[UninstallRun]
; Stop the resident instance so its files can be removed.
Filename: "{app}\tinypaw.exe"; Parameters: "quit"; Flags: runhidden waituntilterminated; RunOnceId: "QuitTinyPaw"

[Code]
const
  EnvKey = 'Environment';
  RunKey = 'Software\Microsoft\Windows\CurrentVersion\Run';

function PathContains(Path, Dir: string): Boolean;
begin
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Path) + ';') > 0;
end;

procedure AddToPath(Dir: string);
var
  Path: string;
begin
  if not RegQueryStringValue(HKCU, EnvKey, 'Path', Path) then
    Path := '';
  if PathContains(Path, Dir) then
    Exit;
  if (Path <> '') and (Copy(Path, Length(Path), 1) <> ';') then
    Path := Path + ';';
  RegWriteExpandStringValue(HKCU, EnvKey, 'Path', Path + Dir);
end;

procedure RemoveFromPath(Dir: string);
var
  Path: string;
  P: Integer;
begin
  if not RegQueryStringValue(HKCU, EnvKey, 'Path', Path) then
    Exit;
  Path := ';' + Path + ';';
  P := Pos(';' + Uppercase(Dir) + ';', Uppercase(Path));
  if P = 0 then
    Exit;
  Delete(Path, P, Length(Dir) + 1);
  Path := Copy(Path, 2, Length(Path) - 2);
  RegWriteExpandStringValue(HKCU, EnvKey, 'Path', Path);
end;

// An upgrade replaces tinypaw.exe, so ask a running copy to quit first.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
begin
  if FileExists(ExpandConstant('{app}\tinypaw.exe')) then
    Exec(ExpandConstant('{app}\tinypaw.exe'), 'quit', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Result := '';
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddToPath(ExpandConstant('{app}'));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: string;
begin
  if CurUninstallStep = usUninstall then
  begin
    RemoveFromPath(ExpandConstant('{app}'));
    // Written by Settings > Controls > Start TinyPaw when I sign in.
    RegDeleteValue(HKCU, RunKey, 'TinyPaw');
  end;
  if CurUninstallStep = usPostUninstall then
  begin
    DataDir := ExpandConstant('{userappdata}\TinyPaw');
    if DirExists(DataDir) and not UninstallSilent then
      if MsgBox('Also delete your TinyPaw settings and imported pointers?' + #13#10#13#10 +
                DataDir + #13#10#13#10 +
                'Choose No to keep them for a later reinstall.',
                mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
        DelTree(DataDir, True, True, True);
  end;
end;
