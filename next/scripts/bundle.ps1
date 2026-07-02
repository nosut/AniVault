$ErrorActionPreference = "Stop"

$cargoPath = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
if (-not (Test-Path -LiteralPath $cargoPath)) {
  $cargo = Get-Command cargo -ErrorAction SilentlyContinue
  if (-not $cargo) { throw "cargo not found" }
  $cargoPath = $cargo.Source
}

$vcvars = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
$tauriArgs = "tauri build --bundles nsis"

Push-Location -LiteralPath "src-tauri"
try {
  if (Test-Path -LiteralPath $vcvars) {
    & cmd.exe /c "call `"$vcvars`" && `"$cargoPath`" $tauriArgs"
  } else {
    & $cargoPath tauri build --bundles nsis
  }
  if ($LASTEXITCODE -ne 0) { throw "Tauri bundle failed with exit code $LASTEXITCODE" }
} finally {
  Pop-Location
}

$bundleRoot = Join-Path (Get-Location) "src-tauri\target\release\bundle"
$artifacts = Get-ChildItem -LiteralPath $bundleRoot -Recurse -File -Include *.exe,*.msi 2>$null
if (-not $artifacts) {
  throw "No installer artifact found under $bundleRoot"
}

"Installer artifacts:"
$artifacts | ForEach-Object { $_.FullName }
