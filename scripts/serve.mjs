import http from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';
const root = path.resolve(import.meta.dirname, '..', process.argv.includes('--dist') ? 'dist' : '.');
const types = { '.html':'text/html; charset=utf-8','.mjs':'text/javascript; charset=utf-8','.js':'text/javascript; charset=utf-8','.css':'text/css; charset=utf-8','.json':'application/json; charset=utf-8','.webmanifest':'application/manifest+json','.svg':'image/svg+xml','.png':'image/png','.jpg':'image/jpeg','.webp':'image/webp','.woff2':'font/woff2','.md':'text/plain; charset=utf-8' };
const server = http.createServer(async (req,res) => {
  try {
    const urlPath = decodeURIComponent(new URL(req.url,'http://localhost').pathname);
    const rel = urlPath === '/' ? 'index.html' : urlPath.replace(/^\/+/, '');
    let file = path.resolve(root, rel);
    if (!file.startsWith(root + path.sep) && file !== path.join(root,'index.html')) throw new Error('outside');
    if (!process.argv.includes('--dist') && !rel.startsWith('src/') && !rel.startsWith('index.html')) file = path.resolve(root,'public',rel);
    if (!(await stat(file)).isFile()) throw new Error('missing');
    res.writeHead(200,{'Content-Type': types[path.extname(file)] || 'application/octet-stream','Cache-Control':'no-cache'});
    res.end(await readFile(file));
  } catch { res.writeHead(404); res.end('Not found'); }
});
const port = Number(process.env.PORT || 5173);
server.listen(port,'127.0.0.1',()=>console.log(`Local: http://127.0.0.1:${port}`));
