const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = process.env.PORT || 3000;
const HOST = process.env.HOST || '127.0.0.1';
const OLLAMA_HOST = process.env.OLLAMA_HOST || 'http://127.0.0.1:11434';
const PUBLIC_DIR = path.join(__dirname, 'public');
const DEFAULT_INSTRUCTION = 'Clean up this raw speech-to-text transcript into readable text while preserving the speaker\'s intent.';

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

function buildRefinePrompt(transcript, instruction) {
  return [
    instruction || DEFAULT_INSTRUCTION,
    '',
    'Raw transcript:',
    transcript,
    '',
    'Output only the cleaned dictation text.'
  ].join('\n');
}

async function fetchOllamaModels(fetchImpl, ollamaHost) {
  const response = await fetchImpl(`${ollamaHost}/api/tags`);
  if (!response.ok) {
    throw new Error(`Ollama /api/tags failed with status ${response.status}`);
  }

  const data = await response.json();
  return (data.models || []).map((item) => item.name);
}

async function handleApi(req, res, deps) {
  const { fetchImpl, ollamaHost } = deps;

  if (req.method === 'GET' && req.url === '/api/health') {
    try {
      const models = await fetchOllamaModels(fetchImpl, ollamaHost);
      sendJson(res, 200, {
        ok: true,
        ollamaHost,
        modelCount: models.length
      });
    } catch (error) {
      sendJson(res, 503, {
        ok: false,
        ollamaHost,
        error: error.message
      });
    }
    return true;
  }

  if (req.method === 'GET' && req.url === '/api/models') {
    try {
      const models = await fetchOllamaModels(fetchImpl, ollamaHost);
      sendJson(res, 200, {
        ok: true,
        models
      });
    } catch (error) {
      sendJson(res, 503, {
        ok: false,
        error: error.message
      });
    }
    return true;
  }

  if (req.method === 'POST' && req.url === '/api/refine') {
    try {
      const body = await readJsonBody(req);
      const model = (body.model || '').trim();
      const transcript = (body.transcript || '').trim();
      const instruction = (body.instruction || '').trim();

      if (!model) {
        sendJson(res, 400, {
          ok: false,
          error: 'Missing model'
        });
        return true;
      }

      if (!transcript) {
        sendJson(res, 400, {
          ok: false,
          error: 'Missing transcript'
        });
        return true;
      }

      const prompt = buildRefinePrompt(transcript, instruction);
      const response = await fetchImpl(`${ollamaHost}/api/generate`, {
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
      sendJson(res, 200, {
        ok: true,
        text: data.response || ''
      });
    } catch (error) {
      sendJson(res, 500, {
        ok: false,
        error: error.message
      });
    }

    return true;
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
  const deps = {
    fetchImpl: options.fetchImpl || fetch,
    ollamaHost: options.ollamaHost || OLLAMA_HOST,
    publicDir: options.publicDir || PUBLIC_DIR
  };

  return http.createServer(async (req, res) => {
    try {
      const apiHandled = await handleApi(req, res, deps);
      if (apiHandled) return;

      const targetPath = req.url === '/'
        ? path.join(deps.publicDir, 'index.html')
        : safePublicPath(req.url, deps.publicDir);

      if (!targetPath) {
        res.writeHead(400);
        res.end('Bad Request');
        return;
      }

      fs.readFile(targetPath, (err, file) => {
        if (err) {
          if (req.url !== '/') {
            fs.readFile(path.join(deps.publicDir, 'index.html'), (fallbackErr, fallback) => {
              if (fallbackErr) {
                res.writeHead(404);
                res.end('Not Found');
                return;
              }
              res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
              res.end(fallback);
            });
            return;
          }

          res.writeHead(404);
          res.end('Not Found');
          return;
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
}

if (require.main === module) {
  const server = createServer();
  server.listen(PORT, HOST, () => {
    console.log(`Dictation starter running at http://${HOST}:${PORT}`);
    console.log(`Using Ollama host: ${OLLAMA_HOST}`);
  });
}

module.exports = {
  buildRefinePrompt,
  createServer,
  fetchOllamaModels,
  getContentType,
  safePublicPath
};
