const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = process.env.PORT || 3000;
const HOST = process.env.HOST || '127.0.0.1';
const PUBLIC_DIR = path.join(__dirname, 'public');

function getContentType(filePath) {
  const ext = path.extname(filePath).toLowerCase();
  if (ext === '.html') return 'text/html; charset=utf-8';
  if (ext === '.css') return 'text/css; charset=utf-8';
  if (ext === '.js') return 'application/javascript; charset=utf-8';
  if (ext === '.json') return 'application/json; charset=utf-8';
  if (ext === '.svg') return 'image/svg+xml';
  if (ext === '.png') return 'image/png';
  if (ext === '.jpg' || ext === '.jpeg') return 'image/jpeg';
  return 'text/plain; charset=utf-8';
}

function safePublicPath(urlPath, publicDir = PUBLIC_DIR) {
  const decoded = decodeURIComponent(urlPath.split('?')[0]);
  const normalized = path.normalize(decoded).replace(/^\/+/, '');
  const resolved = path.join(publicDir, normalized);

  if (!resolved.startsWith(publicDir)) {
    return null;
  }

  return resolved;
}

function createServer(options = {}) {
  const publicDir = options.publicDir || PUBLIC_DIR;

  return http.createServer((req, res) => {
    if (!req.url) {
      res.writeHead(400, { 'Content-Type': 'text/plain; charset=utf-8' });
      res.end('Bad Request');
      return;
    }

    if (req.url.startsWith('/api/')) {
      res.writeHead(404, { 'Content-Type': 'application/json; charset=utf-8' });
      res.end(JSON.stringify({ ok: false, error: 'No API routes are enabled in dictation-only mode.' }));
      return;
    }

    const targetPath = req.url === '/'
      ? path.join(publicDir, 'index.html')
      : safePublicPath(req.url, publicDir);

    if (!targetPath) {
      res.writeHead(400, { 'Content-Type': 'text/plain; charset=utf-8' });
      res.end('Bad Request');
      return;
    }

    fs.readFile(targetPath, (err, file) => {
      if (err) {
        if (req.url !== '/') {
          fs.readFile(path.join(publicDir, 'index.html'), (fallbackErr, fallback) => {
            if (fallbackErr) {
              res.writeHead(404, { 'Content-Type': 'text/plain; charset=utf-8' });
              res.end('Not Found');
              return;
            }
            res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
            res.end(fallback);
          });
          return;
        }

        res.writeHead(404, { 'Content-Type': 'text/plain; charset=utf-8' });
        res.end('Not Found');
        return;
      }

      res.writeHead(200, {
        'Content-Type': getContentType(targetPath)
      });
      res.end(file);
    });
  });
}

if (require.main === module) {
  const server = createServer();
  server.listen(PORT, HOST, () => {
    console.log(`Dictation app running at http://${HOST}:${PORT}`);
  });
}

module.exports = {
  createServer,
  getContentType,
  safePublicPath
};
