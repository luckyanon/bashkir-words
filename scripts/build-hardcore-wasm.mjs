import { spawnSync } from 'node:child_process';
import { copyFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const crate = new URL('../rust/hardcore/', import.meta.url);
const target = new URL('target/', crate);
const wasmTarget = 'wasm32-unknown-unknown';
const build = spawnSync('cargo', [
  'build', '--locked', '--release', '--target', wasmTarget,
  '--target-dir', fileURLToPath(target),
], { cwd: fileURLToPath(crate), stdio: 'inherit' });
if (build.error) throw build.error;
if (build.status !== 0) process.exit(build.status ?? 1);

const publicDirectory = new URL('../public/wasm/', import.meta.url);
await mkdir(publicDirectory, { recursive: true });
await copyFile(new URL(`${wasmTarget}/release/bashkir_hardcore.wasm`, target),
  new URL('hardcore-layout.wasm', publicDirectory));
