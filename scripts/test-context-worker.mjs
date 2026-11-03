// Development test only. The executable under test receives a PATH without Node.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync, realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';

const executable = resolve(process.argv[2] ?? 'src-tauri/target/release/cerne.exe');
const temporary = realpathSync(mkdtempSync(join(tmpdir(), 'cerne-native-worker-test-')));
const project = join(temporary, 'project');
const { mkdirSync } = await import('node:fs');
mkdirSync(project);
const outside = join(temporary, 'outside.txt');
writeFileSync(outside, 'must not read');
writeFileSync(join(project, 'input.txt'), 'ERROR ação 中文');
const path = process.platform === 'win32'
  ? `${process.env.SystemRoot}\\System32;${process.env.SystemRoot}\\System32\\WindowsPowerShell\\v1.0`
  : '/usr/bin:/bin';
const env = { ...process.env, PATH: path };
for (const key of Object.keys(env)) if (key.toLowerCase() === 'path' && key !== 'PATH') delete env[key];
function run(code, file_content = null) {
  const child = spawnSync(executable, ['--cerne-context-worker'], {
    input: JSON.stringify({ project, roots: [], code, file_content }),
    encoding: 'utf8', env, windowsHide: true, timeout: 30000, maxBuffer: 16 * 1024 * 1024,
  });
  assert.ifError(child.error);
  assert.equal(child.status, 0, child.stderr);
  return { ...JSON.parse(child.stdout), live_output: child.stderr };
}
try {
  // Scan the large string directly: creating 100k objects in an interpreter
  // is a different workload and can exceed the configured execution deadline.
  const large = run("console.log((FILE_CONTENT.match(/ERROR[^\\n]*/g) ?? []).join('\\n'))", `${'INFO normal\n'.repeat(100000)}ERROR ação 中文`);
  assert.equal(large.exit_code, 0, large.stderr);
  assert.equal(large.stdout.trim(), 'ERROR ação 中文');
  assert.ok(large.stdout.length < 100);
  assert.equal(run("console.log(require('fs').readFileSync('input.txt', 'utf8'))").stdout.trim(), 'ERROR ação 中文');
  assert.notEqual(run(`console.log(require('fs').readFileSync(${JSON.stringify(outside)}))`).exit_code, 0);
  assert.match(run("require('node:http')").stderr, /indisponivel/);
  assert.notEqual(run('while (true) {}').exit_code, 0);
  const partial=run("console.log('beforelimit731'); while(true) {}");
  assert.notEqual(partial.exit_code,0);
  assert.match(partial.stdout,/beforelimit731/);
  assert.match(partial.live_output,/beforelimit731/);
  const command = process.platform === 'win32' ? "Write-Output 'native-shell-ok'" : "printf native-shell-ok";
  assert.match(run(`console.log(require('child_process').execSync(${JSON.stringify(command)}))`).stdout, /native-shell-ok/);
  console.log('PASS: 7 real executable checks; large Unicode input, fs permissions, missing Node modules, loop limit, live partial output and shell. Worker PATH has no Node.');
} finally {
  // The only recursive target is the owned folder created by mkdtemp above.
  assert.ok(temporary.startsWith(realpathSync(tmpdir())));
  assert.match(temporary.split(/[\\/]/).at(-1), /^cerne-native-worker-test-/);
  rmSync(temporary, { recursive: true, force: true });
}
