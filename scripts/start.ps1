param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ProtoCraftArgs
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path -LiteralPath $cargo)) { $cargo = (Get-Command cargo -ErrorAction Stop).Source }
$cargoArgs = @('run', '--release', '--locked', '-p', 'eda-app', '--')
if ($ProtoCraftArgs) { $cargoArgs += $ProtoCraftArgs }
& $cargo @cargoArgs
if ($LASTEXITCODE -ne 0) { throw 'Não foi possível iniciar o ProtoCraft.' }
