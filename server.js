const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = process.env.PORT || 3000;
const OLLAMA_HOST = process.env.OLLAMA_HOST || 'http://127.0.0.1:11434';
const PUBLIC_DIR = path.join(__dirname, 'public');

function sendJson(res, statusCode, data) {
  res.writeHead(statusCode, {
    'Content-Type': 'application/json; charset=utf-8'
  });
  res.end(JSON.stringify(data));
}

function readJsonBody(req) {
  return new Promise((resolve, reject) => {
    let raw = '';

    req.on('data', (chunk) => {
      raw += chunk;
      if (raw.length > 1_000_000) {
        reject(new Error('Request body too large'));
        req.destroy();
      }
    });

    req.on('end', () => {
      try {
        resolve(raw ? JSON.parse(raw) : {});
      } catch {
        reject(new Error('Invalid JSON body'));
      }
    });

    req.on('error', reject);
  });
}

async function fetchOllamaModels() {
  const response = await fetch(`${OLLAMA_HOST}/api/tags`);
  if (!response.ok) {
    throw new Error(`Ollama /api/tags failed with status ${response.status}`);
  }

  const data = await response.json();
  return (data.models || []).map((item) => item.name);
}

async function handleApi(req, res) {
  if (req.method === 'GET' && req.url === '/api/health') {
    try {
      const models = await fetchOllamaModels();
      return sendJson(res, 200, {
        ok: true,
        ollamaHost: OLLAMA_HOST,
        modelCount: models.length
      });
    } catch (error) {
      return sendJson(res, 503, {
        ok: false,
        ollamaHost: OLLAMA_HOST,
        error: error.message
      });
    }
  }

  if (req.method === 'GET' && req.url === '/api/models') {
    try {
      const models = await fetchOllamaModels();
      return sendJson(res, 200, {
        ok: true,
        models
      });
    } catch (error) {
      return sendJson(res, 503, {
        ok: false,
        error: error.message
      });
    }
  }

  if (req.method === 'POST' && req.url === '/api/refine') {
    try {
      const body = await readJsonBody(req);
      const model = (body.model || '').trim();
      const transcript = (body.transcript || '').trim();
      const instruction = (body.instruction || '').trim();

      if (!model) {
        return sendJson(res, 400, {
          ok: false,
          error: 'Missing model'
        });
      }

      if (!transcript) {
        return sendJson(res, 400, {
          ok: false,
          error: 'Missing transcript'
        });
      }

      const prompt = [
        instruction || 'Clean up this raw speech-to-text transcript into readable text while preserving the speaker\'s intent.',
        '',
        'Raw transcript:',
        transcript,
        '',
        'Output only the cleaned dictation text.'
      ].join('\n');

      const response = await fetch(`${OLLAMA_HOST}/api/generate`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          model,
          prompt,
          stream: false,
          options: {
            temperature: 0.2
          }
        })
      });

      if (!response.ok) {
        const errText = await response.text();
        throw new Error(`Ollama /api/generate failed (${response.status}): ${errText}`);
      }

      const data = await response.json();
      return sendJson(res, 200, {
        ok: true,
        text: data.response || ''
      });
    } catch (error) {
      return sendJson(res, 500, {
        ok: false,
        error: error.message
      });
    }
  }

  return false;
}

function getContentType(filePath) {
  const ext = path.extname(filePath).toLowerCase();
  if (ext === '.html') return 'text/html; charset=utf-8';
  if (ext === '.css') return 'text/css; charset=utf-8';
  if (ext === '.js') return 'application/javascript; charset=utf-8';
  if (ext === '.json') return 'application/json; charset=utf-8';
  return 'text/plain; charset=utf-8';
}

function safePublicPath(urlPath) {
  const decoded = decodeURIComponent(urlPath.split('?')[0]);
  const normalized = path.normalize(decoded).replace(/^\/+/, '');
  const resolved = path.join(PUBLIC_DIR, normalized);

  if (!resolved.startsWith(PUBLIC_DIR)) {
    return null;
  }

  return resolved;
}

const server = http.createServer(async (req, res) => {
  try {
    const apiHandled = await handleApi(req, res);
    if (apiHandled !== false) return;

    const targetPath = req.url === '/' ? path.join(PUBLIC_DIR, 'index.html') : safePublicPath(req.url);

    if (!targetPath) {
      res.writeHead(400);
      return res.end('Bad Request');
    }

    fs.readFile(targetPath, (err, file) => {
      if (err) {
        if (req.url !== '/') {
          fs.readFile(path.join(PUBLIC_DIR, 'index.html'), (fallbackErr, fallback) => {
            if (fallbackErr) {
              res.writeHead(404);
              return res.end('Not Found');
            }
            res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
            return res.end(fallback);
          });
          return;
        }

        res.writeHead(404);
        return res.end('Not Found');
      }

      res.writeHead(200, {
        'Content-Type': getContentType(targetPath)
      });
      res.end(file);
    });
  } catch (error) {
    res.writeHead(500, { 'Content-Type': 'text/plain; charset=utf-8' });
    res.end(`Server error: ${error.message}`);
  }
});

server.listen(PORT, () => {
  console.log(`Dictation starter running at http://localhost:${PORT}`);
  console.log(`Using Ollama host: ${OLLAMA_HOST}`);
});
