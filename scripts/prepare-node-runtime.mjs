// Build-time only: ship a private Node executable, never require a user install.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync, mkdirSync, mkdtempSync, cpSync, chmodSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const pinned = JSON.parse(readFileSync(join(repo, "scripts/node-runtime.json"), "utf8").replace(/^\uFEFF/, ""));
const platform = process.platform === "win32" ? "win" : process.platform;
const targetArch = process.env.TAURI_ENV_ARCH || process.arch;
const arch = { x86_64: "x64", aarch64: "arm64" }[targetArch] || targetArch;
// The macOS installer is universal; both runtime architectures must be present.
const targets = platform === "darwin" ? ["darwin-x64", "darwin-arm64"] : [`${platform}-${arch}`];
const root = join(repo, "src-tauri/resources/context-mode/runtime/node");
const digest = data => createHash("sha256").update(data).digest("hex");
async function download(url) {
  const response = await fetch(url, { signal: AbortSignal.timeout(180_000) });
  if (!response.ok) throw new Error(`Download failed: ${response.status} ${url}`);
  return Buffer.from(await response.arrayBuffer());
}

for (const target of targets) {
  const archive = pinned.archives[target];
  if (!archive) throw new Error(`Unsupported bundled Node target: ${target}`);
  const destination = join(root, target);
  const binaryName = target.startsWith("win-") ? "node.exe" : "node";
  const binary = join(destination, binaryName);
  const manifestPath = join(destination, "runtime.json");
  if (existsSync(binary) && existsSync(manifestPath) && existsSync(join(destination, "LICENSE"))) {
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
    execFileSync("tar", ["-xf", archivePath, "-C", scratch, binaryRelative, `${folder}/LICENSE`], { windowsHide: true });
    mkdirSync(destination, { recursive: true });
    cpSync(join(scratch, binaryRelative), binary);
    cpSync(join(scratch, folder, "LICENSE"), join(destination, "LICENSE"));
    if (!target.startsWith("win-")) chmodSync(binary, 0o755);
    writeFileSync(manifestPath, JSON.stringify({ version: pinned.version, target, source: url,
      archiveSha256: archive.sha256, binarySha256: digest(readFileSync(binary)) }, null, 2));
  } finally {
    if (!resolve(scratch).startsWith(resolve(prefix))) throw new Error("Unexpected temporary directory");
    rmSync(scratch, { recursive: true, force: true });
  }
}
