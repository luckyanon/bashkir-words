import {fileURLToPath} from 'node:url';
import {writeRuntimeDictionary} from './runtime-data.mjs';
import { build } from 'vite';
await build({
  root: fileURLToPath(new URL('../',import.meta.url)),
  base: './',
  plugins: [{ name: 'native-entry', transformIndexHtml: { order: 'pre', handler: html => html.replace('./src/main.mjs', './src/native-entry.mjs') } }],
  build: { outDir: 'dist-android', target: 'es2022', emptyOutDir: true },
});

await writeRuntimeDictionary(fileURLToPath(new URL('../dist-android/',import.meta.url)));
