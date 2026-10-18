import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fixture } from './fixtures/mock-gateway.mjs';

const execute = promisify(execFile);
// Each phase uses a different real Node process and the official SQLite
// checkpointer. The planner and RPC are explicit offline fixtures; funds never
// leave these generated, unfunded keys or reach a public RPC.
const child = `
  import {readFileSync} from 'node:fs';
  import {AIMessage,ToolMessage} from '@langchain/core/messages';
  import {CovenantX402Client,SOLANA_GENESIS} from ${JSON.stringify(new URL('../index.mjs', import.meta.url).href)};
  import {CovenantSolanaKeypairWalletProvider} from ${JSON.stringify(new URL('../agentkit.mjs', import.meta.url).href)};
  import {runSupportDemo,SAMPLE_TICKET} from ${JSON.stringify(new URL('../langgraph-demo.mjs', import.meta.url).href)};
  const config=JSON.parse(readFileSync(process.argv[1],'utf8'));
  const walletProvider=new CovenantSolanaKeypairWalletProvider({keypair:Uint8Array.from(config.keypair),rpcUrl:config.client.rpcUrl,genesisHash:SOLANA_GENESIS});
  const client=new CovenantX402Client({...config.client,walletProvider});
  const planner={
    bindTools(tools,options){if(tools.length!==1||options.tool_choice!=='auto')throw Error('bad planner tool registration');return this;},
    async invoke(messages){return messages.some(m=>m instanceof ToolMessage)
      ?new AIMessage('Sample ticket triage and draft prepared for human review.')
      :new AIMessage({content:'',tool_calls:[{type:'tool_call',id:'scripted-model-tool-call',name:'covenant_inference',args:{prompt:SAMPLE_TICKET}}]});}
  };
  try{const result=await runSupportDemo({...config.demo,client,planner,phase:process.argv[2]});console.log(JSON.stringify({phase:result.phase,status:result.purchase?.status??'ok'}));}
  finally{client.close();}
`;

for (const pendingFirst of [false, true]) {
  test(`persistent support demo runs across processes and replays exact result${pendingFirst ? ' after pending recovery' : ''}`, async () => {
    const f = await fixture({ pendingFirst });
    const directory = mkdtempSync(join(tmpdir(), 'covenant-langgraph-offline-'));
    const file = join(directory, 'config.json');
    const { walletProvider: _wallet, ...client } = f.config;
    writeFileSync(file, JSON.stringify({ client, keypair: f.walletKeypair,
      demo: { requestId: randomUUID(), threadId: randomUUID(), model: 'covenant-test', outputDirectory: join(directory, 'run') } }), { mode: 0o600 });
    const run = phase => execute(process.execPath, ['--input-type=module', '-e', child, file, phase], { cwd: new URL('..', import.meta.url), timeout: 30000 });
    const record = phase => JSON.parse(readFileSync(join(directory, 'run', phase + '.json'), 'utf8'));
    try {
      await run('plan'); assert.deepEqual(record('plan').next, ['tools']);
      assert.equal(f.counts().requests, 0, 'planning must not contact the paid client');
      if (pendingFirst) {
        await assert.rejects(run('purchase'), /purchase_pending_resume_original_checkpoint/);
        const before = f.counts();
        await assert.rejects(run('finish'), /completed|pending|purchase/);
        assert.deepEqual(f.counts(), before, 'finish cannot dispatch or hide a pending purchase');
      }
      await run('purchase'); const accepted = record('purchase');
      assert.equal(accepted.purchase.status, 'completed');
      assert.equal(f.counts().quotes, 1);
      assert.equal(f.counts().sends, pendingFirst ? 2 : 1);
      if (pendingFirst) assert.equal(f.counts().payments[0], f.counts().payments[1], 'pending recovery reuses the exact signed authorization');
      const before = f.counts();
      await run('replay'); assert.deepEqual(record('replay').purchase, accepted.purchase);
      assert.deepEqual(f.counts(), before, 'completed replay performs no network requests');
      await run('finish'); assert.deepEqual(record('finish').next, []);
      assert.deepEqual(f.counts(), before);
      const config = JSON.parse(readFileSync(file, 'utf8')); config.demo.requestId = randomUUID();
      writeFileSync(file, JSON.stringify(config), { mode: 0o600 });
      await assert.rejects(run('purchase'), /demo_identity_changed/);
      assert.deepEqual(f.counts(), before);
    } finally { await f.cleanup(); rmSync(directory, { recursive: true, force: true }); }
  });
}
