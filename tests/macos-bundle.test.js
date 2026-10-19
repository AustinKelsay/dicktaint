import { afterEach, expect, test } from "bun:test";
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { version } from "../package.json";

const temporaryDirectories = [];
afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) rmSync(directory, { recursive: true, force: true });
});

function fixture() {
  const directory = mkdtempSync(join(tmpdir(), "murmur-bundle-test-"));
  temporaryDirectories.push(directory);
  const app = join(directory, "Murmur.app");
  mkdirSync(join(app, "Contents", "MacOS"), { recursive: true });
  const plist = {
    CFBundleIdentifier: "com.plebdev.dicktaint",
    CFBundleExecutable: "dicktaint",
    CFBundleName: "Murmur",
    CFBundleDisplayName: "Murmur",
    CFBundleVersion: version,
    CFBundleShortVersionString: version,
    CFBundlePackageType: "APPL",
    NSMicrophoneUsageDescription: "Murmur uses your microphone to transcribe your voice on your Mac.",
  };
  writeFileSync(join(app, "Contents", "Info.plist"), JSON.stringify(plist));
  Bun.spawnSync(["plutil", "-convert", "xml1", join(app, "Contents", "Info.plist")]);
  const executable = join(app, "Contents", "MacOS", "dicktaint");
  const compilation = Bun.spawnSync(["xcrun", "clang", "-x", "c", "-o", executable, "-"], {
    stdin: Buffer.from("int main(void) { return 0; }\n"),
  });
  expect(compilation.exitCode).toBe(0);
  cpSync(executable, join(app, "Contents", "MacOS", "whisper-cli"));
  function sign(entitlements = true) {
    const child = Bun.spawnSync(["codesign", "--force", "--sign", "-", join(app, "Contents", "MacOS", "whisper-cli")]);
    expect(child.exitCode).toBe(0);
    const command = ["codesign", "--force", "--sign", "-"];
    if (entitlements) command.push("--entitlements", "src-tauri/dicktaint.entitlements");
    const result = Bun.spawnSync([...command, app]);
    expect(result.exitCode).toBe(0);
  }
  return { app, sign };
}

function verify(app) {
  const result = Bun.spawnSync(["bash", "scripts/verify-macos-bundle.sh", app]);
  return { status: result.exitCode, output: result.stdout.toString() + result.stderr.toString() };
}

test.skipIf(process.platform !== "darwin")("a valid ad hoc bundle passes as local verification with a distribution limitation", () => {
  const { app, sign } = fixture();
  sign();
  const result = verify(app);
  if (result.status !== 0) console.error(result.output);
  expect(result.status).toBe(0);
  expect(result.output).toContain("LOCAL ONLY");
  expect(result.output).toContain("Bundle verification passed");
});

test.skipIf(process.platform !== "darwin")("a bundle with a stale version fails before signing verification", () => {
  const { app } = fixture();
  Bun.spawnSync(["/usr/libexec/PlistBuddy", "-c", "Set :CFBundleShortVersionString 0.3.11", join(app, "Contents", "Info.plist")]);
  const result = verify(app);
  expect(result.status).toBe(1);
  expect(result.output).toContain(`CFBundleShortVersionString must be '${version}'`);
});

test.skipIf(process.platform !== "darwin")("a missing sidecar fails even when the main executable is valid", () => {
  const { app } = fixture();
  rmSync(join(app, "Contents", "MacOS", "whisper-cli"));
  const result = verify(app);
  expect(result.status).toBe(1);
  expect(result.output).toContain("Missing executable whisper-cli");
});

test.skipIf(process.platform !== "darwin")("a placeholder sidecar cannot pass as a transcription binary", () => {
  const { app } = fixture();
  writeFileSync(join(app, "Contents", "MacOS", "whisper-cli"), "#!/bin/sh\necho placeholder\n");
  const result = verify(app);
  expect(result.status).toBe(1);
  expect(result.output).toContain("whisper-cli is not a Mach-O binary");
});

test.skipIf(process.platform !== "darwin")("a signed bundle without microphone entitlement fails", () => {
  const { app, sign } = fixture();
  sign(false);
  const result = verify(app);
  expect(result.status).toBe(1);
  expect(result.output).toContain("missing the audio-input entitlement");
});

test.skipIf(process.platform !== "darwin")("a bundle changed after signing fails strict verification", () => {
  const { app, sign } = fixture();
  sign();
  mkdirSync(join(app, "Contents", "Resources"), { recursive: true });
  writeFileSync(join(app, "Contents", "Resources", "unsigned.txt"), "changed after signing");
  const result = verify(app);
  expect(result.status).toBe(1);
  expect(result.output).toContain("Bundle signature verification failed");
});
