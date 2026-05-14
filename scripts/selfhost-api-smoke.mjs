#!/usr/bin/env node
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdtemp, rm } from 'node:fs/promises';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..');
const releaseBinary = path.join(repoRoot, 'target', 'release', 'sparklytics');
const smokeBinary = process.env.SPARKLYTICS_SMOKE_BIN || releaseBinary;

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function getFreePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      const port = typeof address === 'object' && address ? address.port : null;
      server.close(() => {
        if (port) {
          resolve(port);
        } else {
          reject(new Error('failed to allocate a free port'));
        }
      });
    });
  });
}

function backendCommand() {
  if (process.env.SPARKLYTICS_SMOKE_BIN || existsSync(releaseBinary)) {
    return { command: smokeBinary, args: [] };
  }
  return { command: 'cargo', args: ['run', '-p', 'sparklytics-server'] };
}

async function waitForHealth(baseUrl, stderr) {
  let lastError;
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      const response = await fetch(`${baseUrl}/health`);
      if (response.status === 200) return;
      lastError = new Error(`health returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await sleep(250);
  }
  throw new Error(`timed out waiting for /health: ${lastError?.message ?? 'unknown'}\n${stderr()}`);
}

async function startServer(authMode, extraEnv = {}, options = {}) {
  const port = await getFreePort();
  const dataDir =
    options.dataDir ?? (await mkdtemp(path.join(os.tmpdir(), `sparklytics-api-smoke-${authMode}-`)));
  const { command, args } = backendCommand();
  const child = spawn(command, args, {
    cwd: repoRoot,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: {
      ...process.env,
      RUST_LOG: process.env.RUST_LOG ?? 'warn',
      SPARKLYTICS_PORT: String(port),
      SPARKLYTICS_AUTH: authMode,
      SPARKLYTICS_HTTPS: 'false',
      SPARKLYTICS_DATA_DIR: dataDir,
      SPARKLYTICS_DUCKDB_MEMORY: process.env.SPARKLYTICS_DUCKDB_MEMORY ?? '1GB',
      SPARKLYTICS_BOOTSTRAP_PASSWORD: 'bootstrap-secret',
      SPARKLYTICS_PUBLIC_URL: `http://127.0.0.1:${port}`,
      SPARKLYTICS_TRACKING_PUBLIC_BASE: `http://127.0.0.1:${port}/_sl`,
      SPARKLYTICS_GEOIP_PATH: '/nonexistent/dbip-city-lite.mmdb',
      ...extraEnv,
    },
  });
  let stderr = '';
  child.stderr.on('data', (chunk) => {
    stderr += chunk.toString();
  });
  const baseUrl = `http://127.0.0.1:${port}`;
  await waitForHealth(baseUrl, () => stderr);
  return { baseUrl, child, dataDir };
}

async function stopServer(server, keepData = false) {
  if (!server) return;
  server.child.kill('SIGTERM');
  await new Promise((resolve) => {
    const timeout = setTimeout(resolve, 3000);
    server.child.once('exit', () => {
      clearTimeout(timeout);
      resolve();
    });
  });
  if (!keepData) {
    await rm(server.dataDir, { recursive: true, force: true });
  }
}

async function readJson(response) {
  const text = await response.text();
  return text ? JSON.parse(text) : null;
}

async function assertLocalMode() {
  let server;
  try {
    server = await startServer('local');
    const { baseUrl } = server;

    let response = await fetch(`${baseUrl}/health`);
    assert.equal(response.status, 200, '/health should return 200');

    response = await fetch(`${baseUrl}/api/auth/status`);
    assert.equal(response.status, 200, 'local auth status should return 200');
    assert.deepEqual(await readJson(response), {
      authenticated: false,
      mode: 'local',
      password_change_required: false,
      setup_required: true,
    });

    response = await fetch(`${baseUrl}/api/auth/setup`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        bootstrap_password: 'bootstrap-secret',
        password: 'correct horse battery staple',
      }),
    });
    assert.equal(response.status, 201, 'first setup should return 201');

    response = await fetch(`${baseUrl}/api/auth/setup`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        bootstrap_password: 'bootstrap-secret',
        password: 'correct horse battery staple',
      }),
    });
    assert.equal(response.status, 410, 'second setup should return 410');

    response = await fetch(`${baseUrl}/api/auth/login`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ password: 'correct horse battery staple' }),
    });
    assert.equal(response.status, 200, 'login should return 200');
    const setCookie = response.headers.get('set-cookie') ?? '';
    assert.match(setCookie, /spk_session=/, 'login should set spk_session');
    assert.match(setCookie, /HttpOnly/i, 'session cookie should be HttpOnly');
    assert.match(setCookie, /SameSite=Strict/i, 'session cookie should be SameSite=Strict');
    assert.doesNotMatch(setCookie, /Secure/i, 'session cookie should not be Secure over HTTP');
    const cookie = setCookie.split(';')[0];

    response = await fetch(`${baseUrl}/api/websites`, { headers: { cookie } });
    assert.equal(response.status, 200, 'GET /api/websites should return 200');
    assert.equal((await readJson(response)).data.length, 0, 'fresh instance should have no websites');

    response = await fetch(`${baseUrl}/api/websites`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', cookie },
      body: JSON.stringify({ name: 'Smoke Site', domain: 'smoke.example.com', timezone: 'UTC' }),
    });
    assert.equal(response.status, 201, 'website creation should return 201');
    const website = (await readJson(response)).data;
    assert.equal(website.tenant_id, null, 'self-hosted website tenant_id should be null');
    assert.match(
      website.tracking_snippet,
      new RegExp(`http://127\\.0\\.0\\.1:${new URL(baseUrl).port}/_sl/s\\.js`),
      'tracking snippet should use SPARKLYTICS_TRACKING_PUBLIC_BASE',
    );

    response = await fetch(`${baseUrl}/s.js`);
    assert.equal(response.status, 200, '/s.js should be served');
    assert.match(response.headers.get('content-type') ?? '', /javascript/i);

    response = await fetch(`${baseUrl}/_sl/s.js`);
    assert.equal(response.status, 200, '/_sl/s.js should be served');
    assert.match(response.headers.get('content-type') ?? '', /javascript/i);

    response = await fetch(`${baseUrl}/e`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        'user-agent': 'SparklyticsApiSmoke/1.0',
      },
      body: JSON.stringify({ website_id: website.id, type: 'pageview', url: '/api-smoke' }),
    });
    assert.equal(response.status, 202, '/e should accept a pageview');

    response = await fetch(`${baseUrl}/api/collect`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        'user-agent': 'SparklyticsApiSmoke/1.0',
      },
      body: JSON.stringify({ website_id: website.id, type: 'pageview', url: '/api-collect-smoke' }),
    });
    assert.equal(response.status, 202, '/api/collect should accept a pageview');

    response = await fetch(`${baseUrl}/_sl/e`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        'user-agent': 'SparklyticsApiSmoke/1.0',
      },
      body: JSON.stringify({ website_id: website.id, type: 'pageview', url: '/first-party-smoke' }),
    });
    assert.equal(response.status, 202, '/_sl/e should accept a pageview');

    const today = new Date().toISOString().slice(0, 10);
    let stats;
    for (let attempt = 0; attempt < 40; attempt += 1) {
      response = await fetch(
        `${baseUrl}/api/websites/${website.id}/stats?start_date=${today}&end_date=${today}`,
        { headers: { cookie } },
      );
      assert.equal(response.status, 200, 'stats should return 200');
      stats = await readJson(response);
      if ((stats.data?.pageviews ?? 0) >= 3) break;
      await sleep(250);
    }
    assert.ok((stats.data?.pageviews ?? 0) >= 3, 'stats should show all ingest aliases');

    const dataDir = server.dataDir;
    await stopServer(server, true);
    server = await startServer('local', {}, { dataDir });
    response = await fetch(`${server.baseUrl}/api/auth/login`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ password: 'correct horse battery staple' }),
    });
    assert.equal(response.status, 200, 'login after restart should return 200');
    const restartCookie = (response.headers.get('set-cookie') ?? '').split(';')[0];
    response = await fetch(`${server.baseUrl}/api/websites`, {
      headers: { cookie: restartCookie },
    });
    assert.equal((await readJson(response)).data.length, 1, 'website should persist after restart');
  } finally {
    await stopServer(server);
  }
}

async function assertPasswordMode() {
  let server;
  try {
    server = await startServer('password', {
      SPARKLYTICS_PASSWORD: 'password-mode-secret',
    });
    let response = await fetch(`${server.baseUrl}/api/auth/status`);
    assert.equal(response.status, 200, 'password auth status should return 200');
    const status = await readJson(response);
    assert.equal(status.mode, 'password');
    assert.equal(status.setup_required, false);
    assert.equal(status.authenticated, false);

    response = await fetch(`${server.baseUrl}/api/auth/login`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ password: 'password-mode-secret' }),
    });
    assert.equal(response.status, 200, 'password mode login should return 200');
    assert.match(response.headers.get('set-cookie') ?? '', /spk_session=/);
  } finally {
    await stopServer(server);
  }
}

async function assertNoneMode() {
  let server;
  try {
    server = await startServer('none');
    const response = await fetch(`${server.baseUrl}/api/auth/status`);
    assert.equal(response.status, 404, 'none mode should not register /api/auth/status');
  } finally {
    await stopServer(server);
  }
}

await assertLocalMode();
await assertPasswordMode();
await assertNoneMode();

console.log(
  'selfhost_api_smoke=pass local_setup=201 second_setup=410 login_cookie=spk_session websites_empty=true tenant_id=null s_js=200 first_party_s_js=200 e=202 collect=202 first_party_e=202 stats_pageviews>=3 password_mode=pass none_status=404 snippet_tracking_public_base=pass persistence=pass',
);
