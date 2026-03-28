const { describe, it, expect } = require('bun:test');
const fs = require('fs');
const path = require('path');

const repoRoot = path.resolve(__dirname, '..');
const tauriConfigPath = path.join(repoRoot, 'src-tauri', 'tauri.conf.json');
const entitlementsPath = path.join(repoRoot, 'src-tauri', 'dicktaint.entitlements');

describe('release config', () => {
  it('points macOS bundle signing at the entitlement plist', () => {
    const tauriConfig = JSON.parse(fs.readFileSync(tauriConfigPath, 'utf8'));
    expect(tauriConfig.bundle.macOS.entitlements).toBe('dicktaint.entitlements');
  });

  it('includes the microphone entitlement required by hardened runtime', () => {
    const entitlements = fs.readFileSync(entitlementsPath, 'utf8');
    expect(entitlements).toContain('<key>com.apple.security.device.audio-input</key>');
    expect(entitlements).toContain('<true/>');
  });
});
