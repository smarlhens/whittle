import { strict as assert } from 'node:assert';
import { mkdtempSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));

const nodeFile = readdirSync(__dirname).find(
  f => f.startsWith('whittle.') && f.endsWith('.node'),
);
if (!nodeFile) {
  console.error('No .node binary found. Run `npx napi build --platform` first.');
  process.exit(1);
}

const napi = require(`./${nodeFile}`);

const work = mkdtempSync(join(tmpdir(), 'whittle-test-'));
const msgPath = join(work, 'COMMIT_EDITMSG');

let passed = 0;
let failed = 0;

function test(name, fn) {
  try {
    fn();
    console.log(`  ok  ${name}`);
    passed += 1;
  } catch (err) {
    console.error(`  FAIL ${name}: ${err.message}`);
    failed += 1;
  }
}

test('fix normalizes a noisy subject', () => {
  writeFileSync(msgPath, 'Chore: Bump A and B.\n');
  const code = napi.runCli(['whittle', 'fix', msgPath]);
  assert.equal(code, 0, 'exit 0');
  assert.equal(readFileSync(msgPath, 'utf8'), 'chore: bump a & b\n');
});

test('fix normalizes scope separator', () => {
  writeFileSync(msgPath, 'fix(api/users): handle null\n');
  const code = napi.runCli(['whittle', 'fix', msgPath]);
  assert.equal(code, 0, 'exit 0');
  assert.equal(readFileSync(msgPath, 'utf8'), 'fix(api-users): handle null\n');
});

test('check passes already-normalized subject', () => {
  writeFileSync(msgPath, 'feat: add thing\n');
  const code = napi.runCli(['whittle', 'check', msgPath]);
  assert.equal(code, 0, 'exit 0');
});

test('check fails on non-conventional subject', () => {
  writeFileSync(msgPath, 'just update stuff\n');
  const code = napi.runCli(['whittle', 'check', msgPath]);
  assert.equal(code, 1, 'exit 1');
});

test('check fails on too-long subject', () => {
  const desc = 'x'.repeat(80);
  writeFileSync(msgPath, `feat: ${desc}\n`);
  const code = napi.runCli(['whittle', 'check', msgPath]);
  assert.equal(code, 1, 'exit 1');
});

console.log(`\n${passed} passed, ${failed} failed`);
process.exit(failed === 0 ? 0 : 1);
