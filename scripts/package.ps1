param(
    [switch]$SkipBuild,
    [string]$Version
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
if (-not $SkipBuild) { & (Join-Path $PSScriptRoot 'build.ps1') }
$workspaceVersion = (Select-String -LiteralPath (Join-Path $root 'Cargo.toml') -Pattern '^version = "([^\"]+)"').Matches[0].Groups[1].Value
if ([string]::IsNullOrWhiteSpace($Version)) { $Version = $workspaceVersion }
if ($Version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') {
    throw "Versão inválida para empacotamento: $Version"
}
$destination = Join-Path $root 'dist\ProtoCraft'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
Copy-Item -LiteralPath 'target\release\protocraft.exe','target\release\protocraft-cli.exe','README.md','README.en.md','README.pt-BR.md','NOTICE.md','LICENSE-MIT','LICENSE-APACHE' -Destination $destination -Force
New-Item -ItemType Directory -Force -Path (Join-Path $destination 'assets') | Out-Null
Copy-Item -LiteralPath 'assets\protocraft-logo.png' -Destination (Join-Path $destination 'assets') -Force
Copy-Item -LiteralPath 'assets\library' -Destination (Join-Path $destination 'assets') -Recurse -Force
New-Item -ItemType Directory -Force -Path (Join-Path $destination 'examples') | Out-Null
$exampleDestination = Join-Path $destination 'examples'
Get-ChildItem -LiteralPath $exampleDestination -File -ErrorAction SilentlyContinue |
    Remove-Item -Force
Copy-Item -Path 'examples\*.protocraft' -Destination (Join-Path $destination 'examples') -Force
Copy-Item -LiteralPath 'docs' -Destination $destination -Recurse -Force
Get-FileHash -LiteralPath (Join-Path $destination 'protocraft.exe'),(Join-Path $destination 'protocraft-cli.exe') -Algorithm SHA256 |
    ForEach-Object { "$($_.Hash.ToLowerInvariant())  $([IO.Path]::GetFileName($_.Path))" } |
    Set-Content -LiteralPath (Join-Path $destination 'SHA256SUMS.txt') -Encoding ASCII
$archive = Join-Path $root ("dist\ProtoCraft-$Version-windows-x64.zip")
Compress-Archive -LiteralPath $destination -DestinationPath $archive -Force
Write-Host "Pacote portátil disponível em: $destination"
Write-Host "ZIP: $archive"
