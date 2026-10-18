import { build } from 'vite';
import { fileURLToPath } from 'node:url';
import { cp, mkdir } from 'node:fs/promises';
const root = fileURLToPath(new URL('../', import.meta.url));
await build({
  root,
  base: './',
  plugins: [{ name: 'iphone-entry', transformIndexHtml: { order: 'pre', handler: html => html
    .replace('./src/main.mjs', './src/ios-entry.mjs')
    .replace(/\s*<script type="module" src="\.\/src\/sw-registration\.mjs"><\/script>/, '')
    .replace(/\s*<link rel="manifest"[^>]*>/, '') } }],
  build: { outDir: 'ios/Web', target: 'safari17', emptyOutDir: true },
});
await mkdir(new URL('../ios/Resources/', import.meta.url), { recursive: true });
await cp(new URL('../android/app/src/main/res/mipmap-xxxhdpi/ic_launcher.png', import.meta.url), new URL('../ios/Resources/AppIcon.png', import.meta.url));
