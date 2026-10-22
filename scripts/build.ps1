param([switch]$SkipTests)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path -LiteralPath $cargo)) { $cargo = (Get-Command cargo -ErrorAction Stop).Source }
function Invoke-Cargo([string[]]$CargoArgs) {
    & $cargo @CargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Falha no Cargo: $($CargoArgs -join ' ')" }
}
Invoke-Cargo -CargoArgs @('fmt','--all','--','--check')
Invoke-Cargo -CargoArgs @('check','--workspace','--locked')
Invoke-Cargo -CargoArgs @('clippy','--workspace','--all-targets','--locked','--','-D','warnings')
if (-not $SkipTests) { Invoke-Cargo -CargoArgs @('test','--workspace','--locked') }
Invoke-Cargo -CargoArgs @('build','--release','--workspace','--locked')
