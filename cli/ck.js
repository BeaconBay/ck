#!/usr/bin/env node
const { spawn } = require('node:child_process');
const { join } = require('node:path');
const { existsSync } = require('node:fs');

const isWindows = process.platform === 'win32';
const exe = isWindows ? '.exe' : '';
const localPath = join(__dirname, '..', 'dist', 'bin', `ck${exe}`);

// Try local installation first, fallback to global PATH
const bin = existsSync(localPath) ? localPath : 'ck';

const child = spawn(bin, process.argv.slice(2), {
  stdio: 'inherit',
  shell: isWindows
});

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
  } else {
    process.exit(code ?? 1);
  }
});
