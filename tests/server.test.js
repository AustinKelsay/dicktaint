const path = require('path');
const { describe, it, expect, beforeEach, afterEach } = require('bun:test');

const {
  createServer,
  getContentType,
  safePublicPath
} = require('../server.js');

async function startServer(options = {}) {
  const server = createServer(options);
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const { port } = server.address();
  return {
    server,
    baseUrl: `http://127.0.0.1:${port}`
  };
}

describe('server utility helpers', () => {
  it('safePublicPath blocks path traversal', () => {
    const base = path.join(process.cwd(), 'public');
    const normalized = safePublicPath('/../../etc/passwd', base);

    expect(normalized.startsWith(base)).toBe(true);
    expect(normalized).toBe(path.join(base, 'etc/passwd'));
    expect(safePublicPath('../outside.txt', base)).toBeNull();
    expect(safePublicPath('/index.html', base)).toBe(path.join(base, 'index.html'));
  });

  it('getContentType maps common extensions', () => {
    expect(getContentType('a.html')).toContain('text/html');
    expect(getContentType('a.css')).toContain('text/css');
    expect(getContentType('a.js')).toContain('application/javascript');
    expect(getContentType('a.txt')).toContain('text/plain');
  });
});

describe('server routes', () => {
  let server;
  let baseUrl;

  beforeEach(async () => {
    const started = await startServer();
    server = started.server;
    baseUrl = started.baseUrl;
  });

  afterEach(async () => {
    await new Promise((resolve) => server.close(resolve));
  });

  it('GET /api/health returns API disabled response', async () => {
    const response = await fetch(`${baseUrl}/api/health`);
    const body = await response.json();

    expect(response.status).toBe(404);
    expect(body.ok).toBe(false);
    expect(body.error).toContain('No API routes are enabled');
  });

  it('serves index.html fallback for unknown routes', async () => {
    const response = await fetch(`${baseUrl}/some/fake/path`);
    const html = await response.text();

    expect(response.status).toBe(200);
    expect(html).toContain('<!doctype html>');
    expect(html).toContain('id="dictationModelSelect"');
    expect(html).toContain('/app.js');
  });
});
