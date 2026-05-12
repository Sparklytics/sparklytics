import { mkdtemp } from 'node:fs/promises';
import { rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { spawn } from 'node:child_process';

const repoRoot = process.cwd();
const dataDir = await mkdtemp(path.join(os.tmpdir(), 'sparklytics-release-smoke-'));
const backendOrigin = process.env.PLAYWRIGHT_RELEASE_BACKEND_URL ?? 'http://127.0.0.1:3000';
const backendUrl = new URL(backendOrigin);
let wrapperShutdown = false;
let cleanedDataDir = false;

function cleanupDataDir() {
  if (cleanedDataDir) return;
  cleanedDataDir = true;
  rmSync(dataDir, { recursive: true, force: true });
}

const child = spawn(
  'cargo',
  ['run', '-p', 'sparklytics-server'],
  {
    cwd: repoRoot,
    stdio: 'inherit',
    env: {
      ...process.env,
      RUST_LOG: process.env.RUST_LOG ?? 'sparklytics=info',
      SPARKLYTICS_AUTH: 'local',
      SPARKLYTICS_HTTPS: 'false',
      SPARKLYTICS_DUCKDB_MEMORY: process.env.SPARKLYTICS_DUCKDB_MEMORY ?? '1GB',
      SPARKLYTICS_DATA_DIR: dataDir,
      SPARKLYTICS_PORT: backendUrl.port || (backendUrl.protocol === 'https:' ? '443' : '80'),
      SPARKLYTICS_PUBLIC_URL: backendOrigin,
    },
  },
);

const stopChild = (signal = 'SIGTERM') => {
  wrapperShutdown = true;
  if (!child.killed) {
    child.kill(signal);
  }
};

process.on('SIGINT', () => stopChild('SIGINT'));
process.on('SIGTERM', () => stopChild('SIGTERM'));
process.on('exit', () => {
  stopChild('SIGTERM');
  cleanupDataDir();
});

child.on('exit', (code, signal) => {
  cleanupDataDir();
  if (signal) {
    process.exit(wrapperShutdown ? 0 : 1);
  }
  process.exit(code ?? 1);
});
