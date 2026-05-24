#!/usr/bin/env node
/*
 * npm install script for ck-search
 *
 * Strategy:
 * 1. Try to download prebuilt binary from GitHub releases
 * 2. If that fails and Cargo is available, build from source
 * 3. If neither works, fail with helpful error message
 */

const { spawnSync } = require('node:child_process');
const { existsSync, mkdirSync, copyFileSync, chmodSync, createWriteStream } = require('node:fs');
const { join } = require('node:path');
const https = require('node:https');
const http = require('node:http');
const os = require('node:os');
const tar = require('tar');

function log(message, step) {
  const prefix = step ? `[@beaconbay/ck-search] [${step}]` : '@beaconbay/ck-search:';
  console.log(`${prefix} ${message}`);
}

function logStep(step, total, message) {
  console.log(`\n[@beaconbay/ck-search] [${step}/${total}] ${message}`);
}

function fail(message) {
  console.error(`\n[@beaconbay/ck-search] ❌ ERROR: ${message}`);
  process.exit(1);
}

function hasCargo() {
  try {
    const result = spawnSync('cargo', ['--version'], { stdio: 'ignore' });
    return result.status === 0;
  } catch {
    return false;
  }
}

function buildFromSource() {
  logStep(2, 2, '🔨 Building from source with cargo...');
  log('This may take a few minutes...', '   ');

  const args = ['build', '--release', '--locked', '--package', 'ck-search'];
  const result = spawnSync('cargo', args, { stdio: 'inherit' });

  if (result.status !== 0) {
    fail('cargo build failed. Please ensure Rust and Cargo are installed: https://www.rust-lang.org/tools/install');
  }

  log('Copying binary to dist/bin/...', '   ');
  const isWindows = process.platform === 'win32';
  const exe = isWindows ? '.exe' : '';
  const builtBinary = join(__dirname, '..', 'target', 'release', `ck${exe}`);

  if (!existsSync(builtBinary)) {
    fail(`Build succeeded but binary not found at ${builtBinary}`);
  }

  const destDir = join(__dirname, '..', 'dist', 'bin');
  mkdirSync(destDir, { recursive: true });

  const dest = join(destDir, `ck${exe}`);
  copyFileSync(builtBinary, dest);

  try {
    chmodSync(dest, 0o755);
  } catch (e) {
    // Windows doesn't support chmod, that's okay
  }

  console.log(`\n[@beaconbay/ck-search] ✅ Built from source successfully!`);
}

function detectTargetTriple() {
  const platform = os.platform();
  const arch = os.arch();

  // Map Node.js platform/arch to Rust target triples
  const targetMap = {
    'linux-x64': 'x86_64-unknown-linux-gnu',
    'darwin-x64': 'x86_64-apple-darwin',
    'darwin-arm64': 'aarch64-apple-darwin',
    'win32-x64': 'x86_64-pc-windows-msvc',
    'win32-arm64': 'aarch64-pc-windows-msvc',
  };

  const key = `${platform}-${arch}`;
  return targetMap[key] || null;
}

function download(url, destPath) {
  return new Promise((resolve, reject) => {
    const client = url.startsWith('https:') ? https : http;

    client.get(url, (res) => {
      // Handle redirects
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        return resolve(download(res.headers.location, destPath));
      }

      if (res.statusCode !== 200) {
        return reject(new Error(`HTTP ${res.statusCode}: ${res.statusMessage}`));
      }

      const file = createWriteStream(destPath);
      res.pipe(file);

      file.on('finish', () => {
        file.close();
        resolve();
      });

      file.on('error', (err) => {
        file.close();
        reject(err);
      });
    }).on('error', reject);
  });
}

async function tryDownloadPrebuilt() {
  logStep(1, 2, '📦 Attempting to download prebuilt binary...');

  const version = require('../package.json').version;
  const target = detectTargetTriple();

  if (!target) {
    log(`Platform ${os.platform()}-${os.arch()} not supported for prebuilt binaries`, '   ');
    return false;
  }

  log(`Detected platform: ${target}`, '   ');

  const isWindows = process.platform === 'win32';
  const assetExt = isWindows ? 'zip' : 'tar.gz';
  const assetName = `ck-${version}-${target}.${assetExt}`;
  const downloadUrl = `https://github.com/BeaconBay/ck/releases/download/${version}/${assetName}`;

  const tmpDir = join(__dirname, '..', 'dist', 'tmp');
  const distBin = join(__dirname, '..', 'dist', 'bin');
  mkdirSync(tmpDir, { recursive: true });
  mkdirSync(distBin, { recursive: true });

  const archivePath = join(tmpDir, assetName);

  try {
    log(`Downloading from GitHub releases...`, '   ');
    log(`URL: ${downloadUrl}`, '   ');
    await download(downloadUrl, archivePath);
    log(`✓ Downloaded ${assetName}`, '   ');
  } catch (e) {
    log(`⚠ Prebuilt download failed: ${e.message}`, '   ');
    return false;
  }

  try {
    log('Extracting archive...', '   ');
    if (isWindows) {
      // Use built-in unzip on Windows
      const result = spawnSync('tar', ['-xf', archivePath, '-C', distBin], {
        stdio: 'pipe',
        shell: true
      });
      if (result.status !== 0) {
        throw new Error('Failed to extract zip archive');
      }
    } else {
      // Use tar for Unix systems
      await tar.x({
        file: archivePath,
        cwd: distBin,
      });
    }

    // Ensure executable permissions
    const exe = isWindows ? '.exe' : '';
    const binaryPath = join(distBin, `ck${exe}`);

    try {
      chmodSync(binaryPath, 0o755);
    } catch (e) {
      // Windows doesn't need chmod
    }

    log('✓ Extracted successfully', '   ');
    console.log(`\n[@beaconbay/ck-search] ✅ Prebuilt binary installed!`);
    return true;
  } catch (e) {
    log(`⚠ Failed to extract prebuilt binary: ${e.message}`, '   ');
    return false;
  }
}

async function main() {
  console.log('\n╭──────────────────────────────────────────────╮');
  console.log('│  @beaconbay/ck-search installation           │');
  console.log('╰──────────────────────────────────────────────╯\n');

  // Skip install in CI environments where ck might already be installed globally
  if (process.env.CI && process.env.SKIP_CK_INSTALL) {
    log('Skipping installation in CI (SKIP_CK_INSTALL=true)');
    return;
  }

  // Try prebuilt binary first
  const downloaded = await tryDownloadPrebuilt();
  if (downloaded) {
    console.log('\n✨ Installation complete! Try: ck --version\n');
    return;
  }

  // Fallback to building from source
  console.log('\n[@beaconbay/ck-search] ℹ️  Prebuilt binary not available, falling back to source build...');

  if (!hasCargo()) {
    fail(
      'No prebuilt binary available for your platform and Cargo not found.\n' +
      'Please either:\n' +
      '  1. Install Rust: https://www.rust-lang.org/tools/install\n' +
      '  2. Or use a platform with prebuilt binaries (Linux x64, macOS x64/arm64, Windows x64)'
    );
  }

  buildFromSource();
  console.log('\n✨ Installation complete! Try: ck --version\n');
}

main().catch((e) => {
  fail(e.message);
});
