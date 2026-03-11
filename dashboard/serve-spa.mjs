// Simple SPA static server: /dashboard/* → out/dashboard/index.html
import http from 'http';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const outDir = path.join(__dirname, 'out');
const PORT = Number(process.env.PORT ?? '3002');
const BACKEND_ORIGIN = process.env.BACKEND_ORIGIN ?? 'http://127.0.0.1:3000';

const MIME = {
  '.html': 'text/html',
  '.js': 'application/javascript',
  '.css': 'text/css',
  '.woff2': 'font/woff2',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.json': 'application/json',
  '.txt': 'text/plain',
};

http.createServer(async (req, res) => {
  const url = req.url.split('?')[0];

  if (url.startsWith('/api/')) {
    const body =
      req.method === 'GET' || req.method === 'HEAD'
        ? undefined
        : Buffer.concat(await Array.fromAsync(req, (chunk) => Buffer.from(chunk)));
    const proxyResponse = await fetch(`${BACKEND_ORIGIN}${req.url}`, {
      method: req.method,
      headers: req.headers,
      body,
      duplex: body ? 'half' : undefined,
    });

    res.writeHead(proxyResponse.status, Object.fromEntries(proxyResponse.headers.entries()));
    if (proxyResponse.body) {
      for await (const chunk of proxyResponse.body) {
        res.write(chunk);
      }
    }
    res.end();
    return;
  }

  // Try the exact path first
  let filePath = path.join(outDir, url);
  if (fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
    const ext = path.extname(filePath);
    res.writeHead(200, { 'Content-Type': MIME[ext] || 'application/octet-stream' });
    fs.createReadStream(filePath).pipe(res);
    return;
  }

  // Try path/index.html
  const indexPath = path.join(filePath, 'index.html');
  if (fs.existsSync(indexPath)) {
    res.writeHead(200, { 'Content-Type': 'text/html' });
    fs.createReadStream(indexPath).pipe(res);
    return;
  }

  // SPA fallback: /dashboard/* → out/dashboard/index.html
  if (url.startsWith('/dashboard')) {
    const fallback = path.join(outDir, 'dashboard', 'index.html');
    res.writeHead(200, { 'Content-Type': 'text/html' });
    fs.createReadStream(fallback).pipe(res);
    return;
  }

  // Everything else → root index.html
  res.writeHead(200, { 'Content-Type': 'text/html' });
  fs.createReadStream(path.join(outDir, 'index.html')).pipe(res);
}).listen(PORT, '127.0.0.1', () => {
  console.log(`SPA server running on http://127.0.0.1:${PORT}`);
});
