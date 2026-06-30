import { describe, expect, it } from 'vitest';
import packageJson from '../package.json';
import tauriConfig from '../src-tauri/tauri.conf.json';

describe('AniVault branding metadata', () => {
  it('uses AniVault package and Tauri identity', () => {
    expect(packageJson.name).toBe('anivault');
    expect(tauriConfig.productName).toBe('AniVault');
    expect(tauriConfig.identifier).toBe('app.anivault.desktop');
    expect(tauriConfig.app.windows?.[0]?.title).toBe('AniVault');
  });
});