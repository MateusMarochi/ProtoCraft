param(
    [string]$Compiler,
    [string]$Version
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
if ([string]::IsNullOrWhiteSpace($Compiler)) {
    $candidates = @(
        (Join-Path $env:ProgramFiles 'Inno Setup 7\ISCC.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 7\ISCC.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 7\ISCC.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe')
    )
    $Compiler = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}
$package = Join-Path $root 'dist\ProtoCraft'
$definition = Join-Path $root 'installer\ProtoCraft.iss'
if (-not (Test-Path -LiteralPath $Compiler)) { throw "Inno Setup compiler não encontrado: $Compiler" }
if (-not (Test-Path -LiteralPath (Join-Path $package 'protocraft.exe'))) {
    throw 'Pacote dist/ProtoCraft ausente. Execute scripts/package.ps1 antes.'
}
$workspaceVersion = (Select-String -LiteralPath (Join-Path $root 'Cargo.toml') -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
if ([string]::IsNullOrWhiteSpace($Version)) { $Version = $workspaceVersion }
if ($Version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') {
    throw "Versão inválida para o instalador: $Version"
}
& $Compiler "/DAppVersion=$Version" $definition
if ($LASTEXITCODE -ne 0) { throw "Falha ao gerar instalador (ISCC exit code $LASTEXITCODE)." }
$installer = Join-Path $root "dist\protocraft-$Version-windows-x64.exe"
if (-not (Test-Path -LiteralPath $installer)) { throw "Instalador não foi criado: $installer" }
Get-FileHash -LiteralPath $installer -Algorithm SHA256 | Format-List
Write-Host "Instalador x64: $installer"
