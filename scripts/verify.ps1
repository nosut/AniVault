$ErrorActionPreference = "Stop"

npm run check
npm run test

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if ($cargo) {
  Push-Location -LiteralPath "src-tauri"
  try {
    cargo test
  } finally {
    Pop-Location
  }
  exit 0
}

$cargoPath = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
if (-not (Test-Path -LiteralPath $cargoPath)) {
  throw "cargo not found on PATH or at $cargoPath"
}

$vcvars = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if (-not (Test-Path -LiteralPath $vcvars)) {
  throw "vcvars64.bat not found at $vcvars"
}

Push-Location -LiteralPath "src-tauri"
try {
  & cmd.exe /c "call `"$vcvars`" && `"$cargoPath`" test"
  if ($LASTEXITCODE -ne 0) {
    throw "cargo test failed with exit code $LASTEXITCODE"
  }
} finally {
  Pop-Location
}
