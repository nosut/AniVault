import { describe, expect, it } from 'vitest';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import packageJson from '../package.json';
import tauriConfig from '../src-tauri/tauri.conf.json';

describe('AniVault branding metadata', () => {
  it('uses AniVault package and Tauri identity', () => {
    expect(packageJson.name).toBe('anivault');
    expect(tauriConfig.productName).toBe('AniVault');
    expect(tauriConfig.identifier).toBe('app.anivault.desktop');
    expect(tauriConfig.app.windows[0]!.title).toBe('AniVault');
  });
});

describe('AniVault brand assets', () => {
  it('includes app banner and icon assets', () => {
    expect(existsSync(resolve('src/assets/banner.png'))).toBe(true);
    expect(existsSync(resolve('src-tauri/icons/icon.png'))).toBe(true);
    expect(existsSync(resolve('src-tauri/icons/icon.ico'))).toBe(true);
  });

  it('includes tracking tray and now playing UI modules', () => {
    expect(existsSync(resolve('src/lib/tray.ts'))).toBe(true);
    expect(existsSync(resolve('src/lib/now-playing.svelte'))).toBe(true);
  });
});
