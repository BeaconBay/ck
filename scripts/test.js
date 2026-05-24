#!/usr/bin/env node
/*
 * Simple smoke test to verify ck binary works after npm install
 */

const { spawnSync } = require('node:child_process');
const { join } = require('node:path');
const { existsSync } = require('node:fs');

const isWindows = process.platform === 'win32';
const exe = isWindows ? '.exe' : '';
const binaryPath = join(__dirname, '..', 'dist', 'bin', `ck${exe}`);

if (!existsSync(binaryPath)) {
  console.error('ERROR: ck binary not found at', binaryPath);
  process.exit(1);
}

// Test that binary runs and shows version
const result = spawnSync(binaryPath, ['--version'], {
  encoding: 'utf8',
  stdio: 'pipe'
});

if (result.status !== 0) {
  console.error('ERROR: ck --version failed');
  console.error('stdout:', result.stdout);
  console.error('stderr:', result.stderr);
  process.exit(1);
}

console.log('✓ ck binary works:', result.stdout.trim());
console.log('✓ npm package installation successful');
