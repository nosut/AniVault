# AniVault Rebrand and Installer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebrand the Phase 0 desktop shell as AniVault and produce a Windows installer that installs beside the existing Taiga application.

**Architecture:** Keep the Phase 0 engine unchanged except for crate/package naming needed for the desktop app identity. Move supplied branding assets into `next/`, display the banner in the Svelte shell, generate a real Windows icon from `Icon.png`, enable Tauri bundling, and build a local installer artifact. Existing C++ Taiga code under `src/` remains untouched.

**Tech Stack:** Tauri v2, Svelte 5, TypeScript, Rust 2021, PowerShell asset conversion, Windows Tauri bundler.

## Global Constraints

- Product name: `AniVault`.
- Install identity: separate app from existing Taiga.
- Tauri identifier: `app.anivault.desktop`.
- Existing Taiga install must not be replaced or modified.
- Use `Icon.png` as the application, window, and installer icon source.
- Use `Banner.png` as the home-screen brand banner.
- Keep Phase 0 engine behavior unchanged.
- Produce a local test installer from Tauri's bundler.
- Existing C++ Taiga source under `src/` must remain untouched.
- Installer may be unsigned for local testing.

---

## File Structure

```text
next/
  package.json                         # Rename package to anivault; add bundle script
  package-lock.json                     # Updated package metadata
  README.md                             # Rename to AniVault
  src/
    App.svelte                          # Display AniVault brand and banner
    assets/banner.png                   # Copied from root Banner.png
  src-tauri/
    Cargo.toml                          # Rename Rust package/lib/bin identity
    Cargo.lock                          # Updated package metadata
    tauri.conf.json                     # Product, identifier, bundle config, icon list
    icons/icon.ico                      # Real ICO generated from root Icon.png
    icons/icon.png                      # Copied root Icon.png source asset
    src/main.rs                         # Use renamed Rust lib crate
    tests/*.rs                          # Update crate imports after Rust lib rename
```

---

### Task 1: Rebrand metadata and Rust crate identity

**Files:**
- Modify: `next/package.json`
- Modify: `next/README.md`
- Modify: `next/src-tauri/Cargo.toml`
- Modify: `next/src-tauri/src/main.rs`
- Modify: `next/src-tauri/tests/event_bus_test.rs`
- Modify: `next/src-tauri/tests/migration_test.rs`
- Modify: `next/src-tauri/tests/secrets_test.rs`
- Modify: `next/src-tauri/tests/storage_test.rs`
- Modify: `next/src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: existing Phase 0 tests and verify script.
- Produces: package/app metadata using `AniVault`, `app.anivault.desktop`, Rust lib crate `anivault_core`, and bundle script `npm run bundle`.

- [ ] **Step 1: Write failing metadata test**

Create `next/src/brand.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import packageJson from '../package.json';
import tauriConfig from '../src-tauri/tauri.conf.json';

describe('AniVault branding metadata', () => {
  it('uses AniVault package and Tauri identity', () => {
    expect(packageJson.name).toBe('anivault');
    expect(tauriConfig.productName).toBe('AniVault');
    expect(tauriConfig.identifier).toBe('app.anivault.desktop');
    expect(tauriConfig.app.windows[0].title).toBe('AniVault');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run from `next`:

```powershell
npm run test -- src/brand.test.ts
```

Expected: FAIL because package/config still say `taiga-next`, `Taiga Next`, and `app.taiga.next`.

- [ ] **Step 3: Update metadata**

Modify `next/package.json`:

```json
{
  "name": "anivault",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "check": "tsc --noEmit",
    "test": "vitest run",
    "verify": "pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/verify.ps1",
    "bundle": "npm run build && pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/bundle.ps1"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.5.0",
    "svelte": "^5.25.0"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^5.0.3",
    "@types/node": "^22.13.14",
    "typescript": "^5.8.2",
    "vite": "^6.2.3",
    "vitest": "^3.0.9"
  }
}
```

Modify `next/src-tauri/Cargo.toml` package/lib/bin identity:

```toml
[package]
name = "anivault"
version = "0.1.0"
description = "AniVault desktop anime library"
authors = ["AniVault Contributors"]
edition = "2021"

[lib]
name = "anivault_core"
crate-type = ["rlib"]

[[bin]]
name = "anivault"
path = "src/main.rs"
```

Keep the existing build-dependencies and dependencies blocks unchanged.

Modify `next/src-tauri/src/main.rs`:

```rust
fn main() {
    anivault_core::run();
}
```

Replace test imports:

```rust
use anivault_core::engine::...
```

Modify `next/src-tauri/tauri.conf.json` core identity and bundling:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "AniVault",
  "version": "0.1.0",
  "identifier": "app.anivault.desktop",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "AniVault",
        "width": 1280,
        "height": 820,
        "minWidth": 960,
        "minHeight": 640
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "icon": ["icons/icon.ico", "icons/icon.png"]
  }
}
```

Modify `next/README.md` first line and description:

```markdown
# AniVault

Clean-room Windows-only anime library and tracker.
```

- [ ] **Step 4: Run metadata test**

Run from `next`:

```powershell
npm run test -- src/brand.test.ts
```

Expected: PASS.

- [ ] **Step 5: Run full verify**

Run from `next`:

```powershell
npm run verify
```

Expected: TypeScript, Vitest, and Rust tests pass.

- [ ] **Step 6: Commit metadata rebrand**

```powershell
git add next/package.json next/package-lock.json next/README.md next/src/brand.test.ts next/src-tauri/Cargo.toml next/src-tauri/Cargo.lock next/src-tauri/src/main.rs next/src-tauri/tests next/src-tauri/tauri.conf.json
git commit -m "feat: rebrand app as anivault"
```

---

### Task 2: Add supplied brand assets to app UI and Tauri icons

**Files:**
- Create: `next/src/assets/banner.png`
- Create/modify: `next/src-tauri/icons/icon.png`
- Modify: `next/src-tauri/icons/icon.ico`
- Modify: `next/src/App.svelte`
- Modify: `next/src/brand.test.ts`

**Interfaces:**
- Consumes: root `Banner.png` and `Icon.png`.
- Produces: app-visible banner and real icon assets used by Tauri bundler.

- [ ] **Step 1: Add failing asset metadata test**

Extend `next/src/brand.test.ts`:

```ts
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

describe('AniVault brand assets', () => {
  it('includes app banner and icon assets', () => {
    expect(existsSync(resolve('src/assets/banner.png'))).toBe(true);
    expect(existsSync(resolve('src-tauri/icons/icon.png'))).toBe(true);
    expect(existsSync(resolve('src-tauri/icons/icon.ico'))).toBe(true);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run from `next`:

```powershell
npm run test -- src/brand.test.ts
```

Expected: FAIL because `src/assets/banner.png` and `src-tauri/icons/icon.png` do not exist yet.

- [ ] **Step 3: Copy assets and generate ICO**

Run from repo root:

```powershell
New-Item -ItemType Directory -Path "next/src/assets" -Force | Out-Null
New-Item -ItemType Directory -Path "next/src-tauri/icons" -Force | Out-Null
Copy-Item -LiteralPath "Banner.png" -Destination "next/src/assets/banner.png" -Force
Copy-Item -LiteralPath "Icon.png" -Destination "next/src-tauri/icons/icon.png" -Force

Add-Type -AssemblyName System.Drawing
$source = [System.Drawing.Bitmap]::new((Resolve-Path "Icon.png"))
try {
  $resized = [System.Drawing.Bitmap]::new($source, [System.Drawing.Size]::new(256, 256))
  try {
    $iconHandle = $resized.GetHicon()
    try {
      $icon = [System.Drawing.Icon]::FromHandle($iconHandle)
      $stream = [System.IO.File]::Open("next/src-tauri/icons/icon.ico", [System.IO.FileMode]::Create)
      try { $icon.Save($stream) } finally { $stream.Dispose(); $icon.Dispose() }
    } finally {
      Add-Type -Namespace Win32 -Name NativeMethods -MemberDefinition '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool DestroyIcon(System.IntPtr handle);'
      [Win32.NativeMethods]::DestroyIcon($iconHandle) | Out-Null
    }
  } finally { $resized.Dispose() }
} finally { $source.Dispose() }
```

- [ ] **Step 4: Display banner in Svelte shell**

Modify `next/src/App.svelte` script:

```svelte
<script lang="ts">
  import bannerUrl from './assets/banner.png';

  const navItems = ['Home', 'Library', 'Watching', 'Calendar', 'Sync', 'Integrations', 'Settings'];
</script>
```

Modify the home section:

```svelte
<section class="home">
  <img class="banner" src={bannerUrl} alt="AniVault" />
  <p class="eyebrow">Foundation build</p>
  <h1>Your premium dark anime vault.</h1>
  <div class="card">
    <span>AniVault Preview</span>
    <strong>Engine scaffold ready for storage, migration, sync, Sonarr integration, and future tracking workflows.</strong>
  </div>
</section>
```

Add CSS:

```css
.banner {
  display: block;
  width: min(34rem, 100%);
  height: auto;
  margin-bottom: 2rem;
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-card);
}
```

Change `.brand` text in markup to `AniVault`.

- [ ] **Step 5: Run asset test and verify**

Run from `next`:

```powershell
npm run test -- src/brand.test.ts
npm run verify
```

Expected: brand test passes and full verify passes.

- [ ] **Step 6: Commit assets and UI brand**

```powershell
git add next/src/App.svelte next/src/assets/banner.png next/src-tauri/icons/icon.png next/src-tauri/icons/icon.ico next/src/brand.test.ts
git commit -m "feat: add anivault brand assets"
```

---

### Task 3: Bundle local Windows installer

**Files:**
- Create: `next/scripts/bundle.ps1`
- Modify: `next/README.md`

**Interfaces:**
- Consumes: `npm run build`, `src-tauri/tauri.conf.json`, generated icons.
- Produces: `npm run bundle` command and at least one installer under `next/src-tauri/target/release/bundle/`.

- [ ] **Step 1: Add bundle script**

Create `next/scripts/bundle.ps1`:

```powershell
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
```

- [ ] **Step 2: Update README with installer command**

Add to `next/README.md`:

```markdown
Build a local unsigned Windows installer:

```powershell
npm run bundle
```

Installer output is written under:

```text
next/src-tauri/target/release/bundle/
```
```

- [ ] **Step 3: Run verification before packaging**

Run from `next`:

```powershell
npm run verify
```

Expected: all checks pass.

- [ ] **Step 4: Build installer**

Run from `next`:

```powershell
npm run bundle
```

Expected: command exits 0 and prints at least one `.exe` or `.msi` installer path.

- [ ] **Step 5: Commit installer script and README**

```powershell
git add next/scripts/bundle.ps1 next/README.md
git commit -m "build: add anivault installer bundle"
```

---

## Final Verification

Run from `next`:

```powershell
npm run verify
npm run build
npm run bundle
```

Expected:
- TypeScript check passes.
- Vitest passes.
- Rust tests pass.
- Vite build passes.
- Tauri bundle produces an installer artifact under `next/src-tauri/target/release/bundle/`.

Run from repo root:

```powershell
git status --short
```

Expected:
- Only intentional untracked root assets remain if they are not committed directly: `Banner.png`, `Icon.png`.
- No `node_modules`, `dist`, `target`, or `gen` folders are tracked.

## Completion Criteria

- App visible name is `AniVault`.
- Tauri product name is `AniVault`.
- Tauri identifier is `app.anivault.desktop`.
- Existing Taiga install identity is not reused.
- Supplied banner appears in the home screen.
- Supplied icon is used for Tauri bundle icon source.
- Local installer artifact path is reported to the user.
