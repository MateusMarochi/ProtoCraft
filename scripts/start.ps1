param([string]$Project)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$executable = Join-Path $root 'target\release\protocraft.exe'
if (Test-Path -LiteralPath $executable) {
    if ($Project) { & $executable $Project } else { & $executable }
} else {
    Set-Location $root
    $cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
    if (-not (Test-Path -LiteralPath $cargo)) { $cargo = (Get-Command cargo -ErrorAction Stop).Source }
    if ($Project) { & $cargo run -p eda-app -- $Project } else { & $cargo run -p eda-app }
    if ($LASTEXITCODE -ne 0) { throw 'Não foi possível iniciar o ProtoCraft.' }
}
