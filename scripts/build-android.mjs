import { build } from 'vite';
await build({
  root: new URL('../', import.meta.url).pathname.replace(/^\/([A-Z]:)/i, '$1'),
  base: './',
  plugins: [{ name: 'native-entry', transformIndexHtml: { order: 'pre', handler: html => html.replace('./src/main.mjs', './src/native-entry.mjs') } }],
  build: { outDir: 'dist-android', target: 'es2022', emptyOutDir: true },
});
