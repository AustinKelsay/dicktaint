const path = require('path');
const { describe, it, expect, beforeEach, afterEach } = require('bun:test');

const {
  buildRefinePrompt,
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

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: {
      'Content-Type': 'application/json'
    }
  });
}

describe('server utility helpers', () => {
  it('buildRefinePrompt includes defaults and transcript', () => {
    const prompt = buildRefinePrompt('hello there', 'Custom instruction');
    expect(prompt).toContain('Custom instruction');
    expect(prompt).toContain('Raw transcript:');
    expect(prompt).toContain('hello there');
  });

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

describe('server api routes', () => {
  let server;
  let baseUrl;
  let fetchCalls;

  beforeEach(async () => {
    fetchCalls = [];

    const fetchImpl = async (url, options = {}) => {
      fetchCalls.push({ url, options });

      if (url.endsWith('/api/tags')) {
        return jsonResponse({
          models: [{ name: 'model-a:latest' }, { name: 'model-b:latest' }]
        });
      }

      if (url.endsWith('/api/generate')) {
        return jsonResponse({ response: 'cleaned output' });
      }

      return new Response('Not Found', { status: 404 });
    };

    const started = await startServer({
      fetchImpl,
      ollamaHost: 'http://unit-test-ollama:11434'
    });

    server = started.server;
    baseUrl = started.baseUrl;
  });

  afterEach(async () => {
    await new Promise((resolve) => server.close(resolve));
  });

  it('GET /api/health returns status and model count', async () => {
    const response = await fetch(`${baseUrl}/api/health`);
    const body = await response.json();

    expect(response.status).toBe(200);
    expect(body.ok).toBe(true);
    expect(body.ollamaHost).toBe('http://unit-test-ollama:11434');
    expect(body.modelCount).toBe(2);
  });

  it('GET /api/models returns model names', async () => {
    const response = await fetch(`${baseUrl}/api/models`);
    const body = await response.json();

    expect(response.status).toBe(200);
    expect(body.ok).toBe(true);
    expect(body.models).toEqual(['model-a:latest', 'model-b:latest']);
  });

  it('POST /api/refine validates required fields', async () => {
    const response = await fetch(`${baseUrl}/api/refine`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({ transcript: 'hello world' })
    });

    const body = await response.json();
    expect(response.status).toBe(400);
    expect(body.ok).toBe(false);
    expect(body.error).toBe('Missing model');
  });

  it('POST /api/refine forwards prompt to ollama and returns text', async () => {
    const response = await fetch(`${baseUrl}/api/refine`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        model: 'model-a:latest',
        transcript: 'this is   dictation text',
        instruction: 'Please clean this.'
      })
    });

    const body = await response.json();
    expect(response.status).toBe(200);
    expect(body.ok).toBe(true);
    expect(body.text).toBe('cleaned output');

    const generateCall = fetchCalls.find((item) => item.url.endsWith('/api/generate'));
    expect(Boolean(generateCall)).toBe(true);

    const generatePayload = JSON.parse(generateCall.options.body);
    expect(generatePayload.model).toBe('model-a:latest');
    expect(generatePayload.prompt).toContain('Please clean this.');
    expect(generatePayload.prompt).toContain('Raw transcript:');
    expect(generatePayload.prompt).toContain('this is   dictation text');
  });

  it('serves index.html fallback for unknown routes', async () => {
    const response = await fetch(`${baseUrl}/some/fake/path`);
    const html = await response.text();

    expect(response.status).toBe(200);
    expect(html).toContain('<!doctype html>');
    expect(html).toContain('id=\"modelSelect\"');
    expect(html).toContain('/dictation-logic.js');
  });
});
