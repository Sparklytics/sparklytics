import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import http from 'node:http';
import net from 'node:net';
import test from 'node:test';

const dashboardDir = new URL('..', import.meta.url);

async function getFreePort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const address = server.address();
  await new Promise((resolve) => server.close(resolve));
  return address.port;
}

async function waitForHttp(url) {
  let lastError;
  for (let attempt = 0; attempt < 40; attempt += 1) {
    try {
      const response = await fetch(url);
      if (response.status < 500) return;
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw lastError ?? new Error(`Timed out waiting for ${url}`);
}

async function startServeSpa({ backendOrigin }) {
  const port = await getFreePort();
  const child = spawn(process.execPath, ['serve-spa.mjs'], {
    cwd: dashboardDir,
    env: {
      ...process.env,
      BACKEND_ORIGIN: backendOrigin,
      PORT: String(port),
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  let output = '';
  child.stdout.on('data', (chunk) => {
    output += chunk.toString();
  });
  child.stderr.on('data', (chunk) => {
    output += chunk.toString();
  });

  try {
    await waitForHttp(`http://127.0.0.1:${port}/dashboard`);
  } catch (error) {
    child.kill('SIGTERM');
    throw new Error(`serve-spa did not start: ${output}\n${error}`);
  }

  return {
    baseUrl: `http://127.0.0.1:${port}`,
    output: () => output,
    stop: async () => {
      if (!child.killed) child.kill('SIGTERM');
      await new Promise((resolve) => {
        const timeout = setTimeout(resolve, 1_000);
        child.once('exit', () => {
          clearTimeout(timeout);
          resolve();
        });
      });
    },
  };
}

async function startFakeBackend(handler) {
  const server = http.createServer(handler);
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const address = server.address();
  return {
    origin: `http://127.0.0.1:${address.port}`,
    close: () => new Promise((resolve) => server.close(resolve)),
  };
}

test('serve-spa returns JSON 502 instead of crashing when backend proxy is unavailable', async () => {
  const unusedBackendPort = await getFreePort();
  const server = await startServeSpa({
    backendOrigin: `http://127.0.0.1:${unusedBackendPort}`,
  });

  try {
    const response = await fetch(`${server.baseUrl}/api/auth/status`);
    assert.equal(response.status, 502);
    assert.deepEqual(await response.json(), {
      error: {
        code: 'backend_unavailable',
        message: 'Sparklytics backend is unavailable',
        field: null,
      },
    });

    const shellResponse = await fetch(`${server.baseUrl}/dashboard`);
    assert.equal(shellResponse.status, 200);
    assert.match(await shellResponse.text(), /Sparklytics/);
  } finally {
    await server.stop();
  }
});

test('serve-spa proxies first-party script and ingest aliases to backend endpoints', async () => {
  const seen = [];
  const backend = await startFakeBackend(async (req, res) => {
    seen.push(req.url);
    if (req.url === '/s.js') {
      res.writeHead(200, { 'Content-Type': 'application/javascript' });
      res.end('/* proxied script */');
      return;
    }
    if (req.url === '/e') {
      for await (const _chunk of req) {
        // drain request body
      }
      res.writeHead(202, { 'Content-Type': 'application/json' });
      res.end('{"ok":true}');
      return;
    }
    res.writeHead(404, { 'Content-Type': 'application/json' });
    res.end('{"error":{"code":"not_found"}}');
  });
  const server = await startServeSpa({ backendOrigin: backend.origin });

  try {
    const scriptResponse = await fetch(`${server.baseUrl}/_sl/s.js`);
    assert.equal(scriptResponse.status, 200);
    assert.match(await scriptResponse.text(), /proxied script/);

    const ingestResponse = await fetch(`${server.baseUrl}/_sl/e`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ website_id: 'site_test', type: 'pageview', url: '/smoke' }),
    });
    assert.equal(ingestResponse.status, 202);

    assert.deepEqual(seen, ['/s.js', '/e']);
  } finally {
    await server.stop();
    await backend.close();
  }
});
