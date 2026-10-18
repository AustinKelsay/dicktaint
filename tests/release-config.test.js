const { describe, it, expect } = require('bun:test');
const fs = require('fs');
const path = require('path');

const repoRoot = path.resolve(__dirname, '..');
const tauriConfigPath = path.join(repoRoot, 'src-tauri', 'tauri.conf.json');
const entitlementsPath = path.join(repoRoot, 'src-tauri', 'dicktaint.entitlements');
const infoPlistPath = path.join(repoRoot, 'src-tauri', 'Info.plist');

describe('release config', () => {
  it('ships Murmur without changing the installed app identity or executable', () => {
    const tauriConfig = JSON.parse(fs.readFileSync(tauriConfigPath, 'utf8'));
    const packageConfig = JSON.parse(fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'));
    const cargoManifest = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');

    expect(tauriConfig.productName).toBe('Murmur');
    expect(tauriConfig.identifier).toBe('com.plebdev.dicktaint');
    expect(tauriConfig.mainBinaryName).toBe('dicktaint');
    expect(packageConfig.name).toBe('dicktaint');
    expect(cargoManifest).toMatch(/^name = "dicktaint"$/m);
    expect(tauriConfig.version).toBe('0.3.12');
    expect(packageConfig.version).toBe('0.3.12');
    expect(cargoManifest).toMatch(/^version = "0\.3\.12"$/m);
    expect(tauriConfig.bundle.externalBin).toEqual(['binaries/whisper-cli']);
    expect(tauriConfig.bundle.macOS.entitlements).toBe('dicktaint.entitlements');
  });

  it('points macOS bundle signing at the entitlement plist', () => {
    const tauriConfig = JSON.parse(fs.readFileSync(tauriConfigPath, 'utf8'));
    expect(tauriConfig.bundle.macOS.entitlements).toBe('dicktaint.entitlements');
  });

  it('opens a compact Murmur workspace with a usable minimum size', () => {
    const tauriConfig = JSON.parse(fs.readFileSync(tauriConfigPath, 'utf8'));
    expect(tauriConfig.app.windows[0]).toMatchObject({
      label: 'main',
      title: 'Murmur',
      width: 900,
      height: 720,
      minWidth: 740,
      minHeight: 560,
      resizable: true,
    });
  });

  it('includes the microphone entitlement required by hardened runtime', () => {
    const entitlements = fs.readFileSync(entitlementsPath, 'utf8');
    expect(entitlements).toContain('<key>com.apple.security.device.audio-input</key>');
    expect(entitlements).toContain('<true/>');
  });

  it('declares Input Monitoring usage for global Fn hold', () => {
    const infoPlist = fs.readFileSync(infoPlistPath, 'utf8');
    expect(infoPlist).toContain('<key>NSInputMonitoringUsageDescription</key>');
    expect(infoPlist).toContain('Input Monitoring');
  });

  it('identifies Murmur and explains the capability in macOS privacy prompts', () => {
    const infoPlist = fs.readFileSync(infoPlistPath, 'utf8');
    expect(infoPlist).toContain('Murmur uses your microphone to transcribe your voice on your Mac.');
    expect(infoPlist).toContain('Murmur uses speech recognition for dictation where supported.');
    expect(infoPlist).toContain('Murmur can use Input Monitoring so holding Fn / Globe starts dictation while another app is focused.');
  });
});
