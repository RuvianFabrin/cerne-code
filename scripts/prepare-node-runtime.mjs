// Build-time only: ship a private Node executable, never require a user install.
// Downloads ONLY the binaries of the platform being built (macOS gets both
// architectures, since its installer carries both and picks one at runtime).
// Output: src-tauri/resources/node-runtime/bin/<target>/node[.exe], used by
// src-tauri/src/node_runtime.rs and the tauri.<platform>.conf.json overlays.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync, mkdirSync, mkdtempSync, cpSync, chmodSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const pinned = JSON.parse(readFileSync(join(repo, "scripts/node-runtime.json"), "utf8").replace(/^﻿/, ""));
const runtimeDir = join(repo, "src-tauri/resources/node-runtime");
const root = join(runtimeDir, "bin");

// Tauri sets TAURI_ENV_PLATFORM / TAURI_ENV_ARCH for beforeBuildCommand and
// beforeDevCommand; outside Tauri we fall back to the machine running the script.
const platform = {
  windows: "win", macos: "darwin", linux: "linux",
}[process.env.TAURI_ENV_PLATFORM] || { win32: "win", darwin: "darwin", linux: "linux" }[process.platform];
const arch = process.env.TAURI_ENV_ARCH || process.arch;
const targets = { win: ["win-x64"], darwin: ["darwin-x64", "darwin-arm64"], linux: ["linux-x64"] }[platform];
if (!targets) throw new Error(`No private Node for platform: ${platform}`);
// Only x64 Linux has a binary wired in the overlay; fail clearly instead of bundling a binary that cannot run.
if (platform === "linux" && !["x86_64", "x64"].includes(arch)) throw new Error(`No private Node for linux ${arch} yet`);

const digest = data => createHash("sha256").update(data).digest("hex");
async function download(url) {
  const response = await fetch(url, { signal: AbortSignal.timeout(180_000) });
  if (!response.ok) throw new Error(`Download failed: ${response.status} ${url}`);
  return Buffer.from(await response.arrayBuffer());
}

// The packages (docx, pdf-lib, pptxgenjs) are pure JavaScript and shared by every platform.
if (!existsSync(join(runtimeDir, "node_modules", "docx"))) {
  console.log("Installing bundled document libraries...");
  execFileSync(process.platform === "win32" ? "npm.cmd" : "npm", ["ci", "--omit=dev"], {
    cwd: runtimeDir, stdio: "inherit", shell: process.platform === "win32",
  });
}

for (const target of targets) {
  const archive = pinned.archives[target];
  if (!archive) throw new Error(`Unsupported bundled Node target: ${target}`);
  const destination = join(root, target);
  const binaryName = target.startsWith("win-") ? "node.exe" : "node";
  const binary = join(destination, binaryName);
  const manifestPath = join(destination, "runtime.json");
  if (existsSync(binary) && existsSync(manifestPath)) {
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    if (manifest.version === pinned.version && manifest.archiveSha256 === archive.sha256 &&
        manifest.binarySha256 === digest(readFileSync(binary))) {
      if (!target.startsWith("win-")) chmodSync(binary, 0o755);
      console.log(`Private Node ${pinned.version} ready (${target})`);
      continue;
    }
  }
  console.log(`Downloading private Node ${pinned.version} (${target}) for the installer...`);
  const url = `https://nodejs.org/dist/v${pinned.version}/${archive.file}`;
  const bytes = await download(url);
  if (digest(bytes) !== archive.sha256) throw new Error(`Node archive checksum mismatch: ${archive.file}`);
  const prefix = join(tmpdir(), "cerne-private-node-");
  const scratch = mkdtempSync(prefix);
  try {
    const archivePath = join(scratch, archive.file);
    writeFileSync(archivePath, bytes);
    const folder = `node-v${pinned.version}-${target}`;
    const binaryRelative = target.startsWith("win-") ? `${folder}/node.exe` : `${folder}/bin/node`;
    // Windows: use the system bsdtar (reads .zip). A Git-for-Windows tar on PATH treats "C:" as a host name.
    const tar = process.platform === "win32" ? join(process.env.SystemRoot || "C:\\Windows", "System32", "tar.exe") : "tar";
    execFileSync(tar, ["-xf", archive.file, binaryRelative, `${folder}/LICENSE`], { cwd: scratch, windowsHide: true });
    mkdirSync(destination, { recursive: true });
    cpSync(join(scratch, binaryRelative), binary);
    cpSync(join(scratch, folder, "LICENSE"), join(runtimeDir, "LICENSE"));
    if (!target.startsWith("win-")) chmodSync(binary, 0o755);
    writeFileSync(manifestPath, JSON.stringify({ version: pinned.version, target, source: url,
      archiveSha256: archive.sha256, binarySha256: digest(readFileSync(binary)) }, null, 2));
  } finally {
    if (!resolve(scratch).startsWith(resolve(prefix))) throw new Error("Unexpected temporary directory");
    rmSync(scratch, { recursive: true, force: true });
  }
}
