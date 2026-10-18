import test from 'node:test';
import assert from 'node:assert/strict';
import {createSmokeRpc,MAINNET_GENESIS} from './smoke-rpc.mjs';
import {validateImage,smokeEnvironment,verifyHealth} from './smoke.mjs';
const revision='a'.repeat(40),image=`ghcr.io/open-covenant/covenant/compute-node-public@sha256:${'b'.repeat(64)}`;
test('container qualification requires immutable package and exact revision',()=>{
 validateImage(image,revision);
 for(const bad of ['ghcr.io/open-covenant/covenant/compute-node-public:latest',image.replace('ghcr.io','example.com'),image+'suffix'])assert.throws(()=>validateImage(bad,revision));
 assert.throws(()=>validateImage(image,revision.slice(0,8)));
});
test('smoke identities are fresh and both paid admission paths are disabled',()=>{
 const first=smokeEnvironment(),second=smokeEnvironment();
 for(const name of ['COVENANT_GATEWAY_ADMISSIONS_ENABLED','COVENANT_X402_OPERATOR_ENABLED','COVENANT_INFERENCE_PROVIDER_ENABLED'])assert.equal(first[name],'false');
 assert.equal(Buffer.from(first.COVENANT_GATEWAY_IDENTITY_B64,'base64').length,32);
 assert.equal(JSON.parse(Buffer.from(first.COVENANT_GATEWAY_FEE_PAYER_KEY_B64,'base64')).length,64);
 assert.notEqual(first.COVENANT_GATEWAY_IDENTITY_B64,second.COVENANT_GATEWAY_IDENTITY_B64);
 assert.notEqual(first.COVENANT_GATEWAY_BRIDGE_SECRET,second.COVENANT_GATEWAY_BRIDGE_SECRET);
 assert.equal(first.USEPOD_TOKEN,undefined);assert.equal(first.COVENANT_COMPUTE_NODE_IDENTITY_B64,undefined);
 assert.equal(first.COVENANT_COMPUTE_COORDINATOR_URL,'http://smoke-rpc:4045/coordinator');
});
test('readiness rejects the wrong build or enabled admissions',()=>{
 const health={status:'ok',service:'covenant-compute-x402',protocol:'x402-v2',revision,admissions_enabled:false};verifyHealth(health,revision);
 assert.throws(()=>verifyHealth({...health,admissions_enabled:true},revision));assert.throws(()=>verifyHealth({...health,revision:'unknown'},revision));
});
test('isolated RPC permits only genesis and counts forbidden dispatch attempts',async t=>{
 assert.throws(()=>createSmokeRpc('devnet'));
 const {server,counts}=createSmokeRpc(MAINNET_GENESIS);await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 t.after(()=>new Promise(resolve=>server.close(resolve)));const url=`http://127.0.0.1:${server.address().port}`;
 const rpc=method=>fetch(url+'/rpc',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:1,method,params:[]})});
 const read=await rpc('getGenesisHash');assert.equal(read.status,200);assert.equal((await read.json()).result,MAINNET_GENESIS);
 for(const method of ['sendTransaction','simulateTransaction','getLatestBlockhash']){const refused=await rpc(method);assert.equal(refused.status,403);await refused.arrayBuffer();}
 const coordinator=await fetch(url+'/coordinator/federation/jobs',{method:'POST',body:'{}'});assert.equal(coordinator.status,503);await coordinator.arrayBuffer();
 assert.deepEqual(counts,{genesis:1,refused:3,coordinator:1});
});
