const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

// Load the actual installer without downloading a binary or requiring npm
// dependencies. Its existing CI skip path prevents installation side effects.
const source = readFileSync(join(__dirname, '../../scripts/install.js'), 'utf8');

function targetFor(platform, arch) {
  return vm.runInNewContext(source + '\ndetectTargetTriple();', {
    require(name) {
      if (name === 'node:os') return { platform: () => platform, arch: () => arch };
      if (name === 'tar') return {};
      return require(name);
    },
    process: { platform, env: { CI: '1', SKIP_CK_INSTALL: '1' } },
    console: { log() {} },
    __dirname: join(__dirname, '../../scripts'),
  });
}

for (const [platform, arch, target] of [
  ['linux', 'x64', 'x86_64-unknown-linux-gnu'],
  ['linux', 'arm64', 'aarch64-unknown-linux-gnu'],
  ['darwin', 'x64', 'x86_64-apple-darwin'],
  ['darwin', 'arm64', 'aarch64-apple-darwin'],
  ['win32', 'x64', 'x86_64-pc-windows-msvc'],
  ['win32', 'arm64', 'aarch64-pc-windows-msvc'],
]) {
  test(`npm installer selects the released binary for ${platform}/${arch}`, () => {
    assert.equal(targetFor(platform, arch), target);
  });
}

test('unsupported platforms do not select a prebuilt archive', () => {
  assert.equal(targetFor('freebsd', 'x64'), null);
});
