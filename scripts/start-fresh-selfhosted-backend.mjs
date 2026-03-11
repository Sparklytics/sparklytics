import { mkdtemp } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { spawn } from 'node:child_process';

const repoRoot = process.cwd();
const dataDir = await mkdtemp(path.join(os.tmpdir(), 'sparklytics-release-smoke-'));
let wrapperShutdown = false;

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
process.on('exit', () => stopChild('SIGTERM'));

child.on('exit', (code, signal) => {
  if (signal) {
    process.exit(wrapperShutdown ? 0 : 1);
  }
  process.exit(code ?? 1);
});
