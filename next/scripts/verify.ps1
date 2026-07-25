$ErrorActionPreference = "Stop"

# Native commands (npm/cargo) do not honour $ErrorActionPreference, so a
# non-zero exit does NOT throw. Check $LASTEXITCODE explicitly after each step,
# otherwise a failing typecheck/test/build would still let verify report success.

npm run check
if ($LASTEXITCODE -ne 0) { throw "typecheck (npm run check) failed with exit code $LASTEXITCODE" }

npm run check:svelte
if ($LASTEXITCODE -ne 0) { throw "svelte-check (npm run check:svelte) failed with exit code $LASTEXITCODE" }

npm run test
if ($LASTEXITCODE -ne 0) { throw "tests (npm run test) failed with exit code $LASTEXITCODE" }

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if ($cargo) {
  Push-Location -LiteralPath "src-tauri"
  try {
    cargo check --tests
    if ($LASTEXITCODE -ne 0) { throw "cargo check --tests failed with exit code $LASTEXITCODE" }
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
  & cmd.exe /c "call `"$vcvars`" && `"$cargoPath`" check --tests"
  if ($LASTEXITCODE -ne 0) {
    throw "cargo check failed with exit code $LASTEXITCODE"
  }
} finally {
  Pop-Location
}
