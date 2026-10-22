param(
    [string]$Compiler = "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$package = Join-Path $root 'dist\ProtoCraft'
$definition = Join-Path $root 'installer\ProtoCraft.iss'
if (-not (Test-Path -LiteralPath $Compiler)) { throw "Inno Setup compiler não encontrado: $Compiler" }
if (-not (Test-Path -LiteralPath (Join-Path $package 'protocraft.exe'))) {
    throw 'Pacote dist/ProtoCraft ausente. Execute scripts/package.ps1 antes.'
}
$version = (Select-String -LiteralPath (Join-Path $root 'Cargo.toml') -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
& $Compiler "/DAppVersion=$version" $definition
if ($LASTEXITCODE -ne 0) { throw "Falha ao gerar instalador (ISCC exit code $LASTEXITCODE)." }
$installer = Join-Path $root "dist\ProtoCraft-$version-Setup-x64.exe"
if (-not (Test-Path -LiteralPath $installer)) { throw "Instalador não foi criado: $installer" }
Get-FileHash -LiteralPath $installer -Algorithm SHA256 | Format-List
Write-Host "Instalador x64: $installer"
