// Build-time only: embute o zip do motor craft-anything no instalador do Windows (modo Anything), para
// o usuário não precisar baixar nada para ligar o modo. Ghidra e Java continuam sendo baixados depois.
// O zip fica em src-tauri/resources/anything-engine/ (fora do git), com versão e SHA-256 fixados em
// scripts/anything-engine.json (os mesmos valores de src-tauri/src/anything/cli.rs; um teste confere).
// Outras plataformas: não faz nada (o motor só tem instalação automática para Windows x64).
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const pinned = JSON.parse(readFileSync(join(repo, "scripts/anything-engine.json"), "utf8").replace(/^﻿/, ""));
const platform = { windows: "win32", macos: "darwin", linux: "linux" }[process.env.TAURI_ENV_PLATFORM] || process.platform;
if (platform !== "win32") {
  console.log("Motor Anything: nada a embutir fora do Windows.");
  process.exit(0);
}

const dir = join(repo, "src-tauri/resources/anything-engine");
const dest = join(dir, pinned.file);
const digest = data => createHash("sha256").update(data).digest("hex");
// Em build de debug (tauri dev) uma falha de rede só avisa: o app baixa o motor quando precisar.
const debug = process.env.TAURI_ENV_DEBUG === "true";

try {
  if (existsSync(dest) && digest(readFileSync(dest)) === pinned.sha256) {
    console.log(`Motor Anything já embutido (${pinned.file}).`);
    process.exit(0);
  }
  mkdirSync(dir, { recursive: true });
  console.log(`Baixando o motor Anything ${pinned.version} para embutir...`);
  const response = await fetch(pinned.url, { signal: AbortSignal.timeout(600_000) });
  if (!response.ok) throw new Error(`Download falhou: ${response.status} ${pinned.url}`);
  const data = Buffer.from(await response.arrayBuffer());
  if (digest(data) !== pinned.sha256) throw new Error("SHA-256 do motor não confere; nada foi gravado.");
  const part = dest + ".part";
  writeFileSync(part, data);
  rmSync(dest, { force: true });
  renameSync(part, dest);
  console.log(`Motor Anything embutido: ${dest}`);
} catch (error) {
  if (debug) {
    console.warn(`AVISO (debug): motor Anything não embutido: ${error.message}`);
    process.exit(0);
  }
  console.error(`Falha ao embutir o motor Anything: ${error.message}`);
  process.exit(1);
}
