#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';

const trackedFiles = execFileSync('git', ['ls-files', '-z'], { encoding: 'utf8' })
  .split('\0')
  .filter(Boolean);

const errors = [];

const forbiddenTrackedPrefixes = ['cloud/', 'docs/', 'marketing/', 'sdk/next/', 'ops/', 'migrations/'];
for (const file of trackedFiles) {
  for (const prefix of forbiddenTrackedPrefixes) {
    if (file.startsWith(prefix)) {
      errors.push(`public root git must not track ${file}`);
    }
  }
}

for (const file of trackedFiles) {
  const name = file.split('/').pop() ?? file;
  if (name.startsWith('.env') && name !== '.env.example') {
    errors.push(`environment file must not be tracked: ${file}`);
  }
  if (/\.(pem|key|p12|pfx)$/i.test(name)) {
    errors.push(`secret material must not be tracked: ${file}`);
  }
}

const existingTrackedFiles = trackedFiles.filter((file) => existsSync(file));

const manifestFiles = existingTrackedFiles.filter((file) =>
  /(^|\/)(Cargo\.toml|Cargo\.lock|package\.json|package-lock\.json|pnpm-lock\.yaml)$/.test(file),
);
const forbiddenManifestPatterns = [
  { pattern: /(^|["'@\s/-])stripe(["'@\s/-]|$)/i, label: 'Stripe dependency' },
  { pattern: /@clerk\//i, label: 'Clerk dependency' },
  { pattern: /(^|["'@\s/-])clickhouse(["'@\s/-]|$)/i, label: 'ClickHouse runtime dependency' },
];

for (const file of manifestFiles) {
  const content = readFileSync(file, 'utf8');
  for (const { pattern, label } of forbiddenManifestPatterns) {
    if (pattern.test(content)) {
      errors.push(`${label} found in ${file}`);
    }
  }
}

const productionFiles = existingTrackedFiles.filter((file) => {
  if (file === 'scripts/check-public-boundary.mjs') return false;
  if (!/\.(rs|ts|tsx|js|mjs|cjs)$/.test(file)) return false;
  if (!/^(crates|dashboard|scripts|docker|\.github)\//.test(file)) return false;
  if (/(^|\/)(tests?|e2e|__tests__|test-results)\//.test(file)) return false;
  if (/\.(test|spec)\.(ts|tsx|js|mjs|cjs)$/.test(file)) return false;
  return true;
});

const forbiddenRuntimePatterns = [
  { pattern: /spk_live_/i, label: 'cloud API key prefix in production runtime' },
  { pattern: /StripeBillingGate|stripe::|@stripe\//i, label: 'Stripe runtime implementation' },
  { pattern: /ClickHouseBackend|clickhouse::|clickhouse-/i, label: 'ClickHouse runtime implementation' },
  { pattern: /Clerk|@clerk\//i, label: 'Clerk runtime implementation' },
];

for (const file of productionFiles) {
  const content = readFileSync(file, 'utf8');
  for (const { pattern, label } of forbiddenRuntimePatterns) {
    if (pattern.test(content)) {
      errors.push(`${label} found in ${file}`);
    }
  }
}

if (errors.length > 0) {
  console.error('Public repository boundary check failed:');
  for (const error of errors) {
    console.error(`- ${error}`);
  }
  process.exit(1);
}

console.log('Public repository boundary check passed.');
