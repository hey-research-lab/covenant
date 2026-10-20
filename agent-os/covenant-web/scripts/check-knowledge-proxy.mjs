// Exercise the actual Next route with side-effect sentinels, without a daemon,
// operator token, Next server, or browser. Run with Node 24 or newer.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { stripTypeScriptTypes } from 'node:module';
import { join } from 'node:path';

const source = await readFile(new URL('../app/api/covenant/[...path]/route.ts', import.meta.url), 'utf8');
// Replace only module bindings with test doubles. Route logic is unchanged.
const executable = stripTypeScriptTypes(source)
  .replace(/^import .+;\s*$/gm, '')
  .replace(/^export /gm, '');
class TestResponse extends Response {
  static json(value, init) { return new TestResponse(JSON.stringify(value), init); }
}
let checks = 0;
for (const demo of ['0', '1']) {
  let tokenReads = 0;
  let upstreamCalls = 0;
  const routes = new Function('readFile', 'homedir', 'join', 'NextResponse', 'process', 'fetch',
    `${executable}\nreturn { GET, POST, PUT, PATCH, DELETE };`)(
    async () => { tokenReads++; throw new Error('operator token must not be read'); },
    () => '/nonexistent-test-home', join, TestResponse,
    { env: { NEXT_PUBLIC_DEMO_MODE: demo } },
    async () => { upstreamCalls++; throw new Error('must not forward'); },
  );
  for (const method of ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']) {
    for (const path of [
      ['knowledge'], ['knowledge', 'status'], ['knowledge', 'projects', 'private-id'],
      ['compute', 'knowledge', 'query'], ['compute.knowledge', 'search'],
      ['%6bnowledge', 'status'], ['knowledge%2fstatus'], ['Knowledge', 'evidence'],
    ]) {
      const response = await routes[method]({ method, text: async () => '{}' }, { params: Promise.resolve({ path }) });
      assert.equal(response.status, 403, `${demo}/${method}/${path.join('/')}`);
      checks++;
    }
  }
  for (const name of ['knowledge.search', 'knowledge.get', 'knowledge.evidence', 'compute.knowledge.search']) {
    const response = await routes.POST({ method: 'POST', text: async () => JSON.stringify({ name, arguments: {} }) },
      { params: Promise.resolve({ path: ['tools', 'call'] }) });
    assert.equal(response.status, 403, `${demo}/${name}`);
    checks++;
  }
  assert.equal(tokenReads, 0);
  assert.equal(upstreamCalls, 0);
}
console.log(`${checks} knowledge proxy boundary checks passed; no operator token reads or upstream calls.`);
