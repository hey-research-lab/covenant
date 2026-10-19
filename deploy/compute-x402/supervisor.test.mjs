import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {spawnSync} from 'node:child_process';

function boot(t,{tariffRuntime=false,env={}}={}){
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'covenant-supervisor-compat-'));
 t.after(()=>fs.rmSync(dir,{recursive:true,force:true}));
 const source=path.join(dir,'deploy','compute-x402'),data=path.join(dir,'data'),calls=path.join(dir,'calls.jsonl');
 fs.mkdirSync(source,{recursive:true});
 fs.copyFileSync(new URL('./supervisor.mjs',import.meta.url),path.join(source,'supervisor.mjs'));
 fs.writeFileSync(path.join(dir,'deploy','compute-canary-watchdog.mjs'),'export function startWatchdog(){return ()=>{};}');
 // Exercise the actual supervisor with the legacy export surface. Filesystem
 // and identity handling are real; child processes and readiness have no network.
 fs.writeFileSync(path.join(source,'runtime.mjs'),`
  import fs from 'node:fs';
  export {privateDirectory,seed,latch,missing,clearStartingLatch,explicitBoolean,operatorPubkey,startInferenceOperator${tariffRuntime?',materializeTariffs':''}} from ${JSON.stringify(new URL('./runtime.mjs',import.meta.url).href)};
  export class ProcessGroup {
   start(name,command,args,env,optional=false){fs.appendFileSync(process.env.TEST_CALLS,JSON.stringify({name,env,optional})+'\\n');}
   stop(code){process.exitCode=code;}
  }
  export async function waitFor(){}
 `);
 const result=spawnSync(process.execPath,[path.join(source,'supervisor.mjs')],{encoding:'utf8',timeout:5000,env:{
  TEST_CALLS:calls,COVENANT_X402_DATA:data,COVENANT_X402_OPERATOR_ENABLED:'false',COVENANT_GATEWAY_ADMISSIONS_ENABLED:'false',
  COVENANT_GATEWAY_IDENTITY_B64:Buffer.alloc(32,7).toString('base64'),COVENANT_GATEWAY_FEE_PAYER_KEY_B64:Buffer.from('[1,2,3]').toString('base64'),
  COVENANT_GATEWAY_BRIDGE_SECRET:'test-secret-'.repeat(4),...env,
 }});
 assert.equal(result.error,undefined);
 assert.equal(result.status,0,result.stderr);
 assert.equal(result.stderr,'');
 const workers=fs.readFileSync(calls,'utf8').trim().split('\n').map(line=>JSON.parse(line));
 assert.deepEqual(workers.map(worker=>worker.name),['payment verifier','inference gateway']);
 return {data,workers,disabled:path.join(data,'gateway','admissions.disabled')};
}

test('observer overlay boots with the legacy runtime export surface and no tariffs',t=>{
 const {disabled}=boot(t);
 assert.equal(fs.existsSync(disabled),false);
});

test('observer overlay refuses every tariff setting but keeps essential recovery workers',t=>{
 for(const key of ['COVENANT_GATEWAY_TARIFFS_JSON','COVENANT_GATEWAY_TARIFFS_FILE','COVENANT_COMPUTE_NODE_TARIFFS_FILE']){
  const {disabled,workers}=boot(t,{env:{COVENANT_X402_OPERATOR_ENABLED:'true',[key]:''}});
  assert.equal(fs.readFileSync(disabled,'utf8'),'Tariffs require the full pricing runtime image; payment recovery only\n');
  for(const {env} of workers)for(const name of ['COVENANT_GATEWAY_TARIFFS_JSON','COVENANT_GATEWAY_TARIFFS_FILE','COVENANT_COMPUTE_NODE_TARIFFS_FILE'])assert.equal(Object.hasOwn(env,name),false);
 }
});

test('full runtime materializes configured tariffs for both financial workers',t=>{
 const tariff={schema_version:1,id:'model',revision:1,model:'model',base_micro_usdc:100,output_cap_micro_usdc_per_million:1000,max_output_tokens:2048,valid_from_ms:1,valid_until_ms:9999999999999};
 const rows=[{native:tariff,retail:{...tariff,base_micro_usdc:200}}];
 const {disabled,workers}=boot(t,{tariffRuntime:true,env:{COVENANT_GATEWAY_TARIFFS_JSON:JSON.stringify(rows)}});
 assert.equal(fs.existsSync(disabled),false);
 for(const {env} of workers){
  assert.deepEqual(JSON.parse(fs.readFileSync(env.COVENANT_GATEWAY_TARIFFS_FILE)),rows);
  assert.deepEqual(JSON.parse(fs.readFileSync(env.COVENANT_COMPUTE_NODE_TARIFFS_FILE)),[tariff]);
  assert.equal(Object.hasOwn(env,'COVENANT_GATEWAY_TARIFFS_JSON'),false);
 }
});
