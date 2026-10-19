import {verifyImageProvenance} from './smoke.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createSmokeRpc,MAINNET_GENESIS} from './smoke-rpc.mjs';
import {validateImage,smokeEnvironment,verifyHealth,fixtureDiagnostics} from './smoke.mjs';
import {closedDiscovery,verifyClosedDiscovery,brandingReads,readSmokeResponse,smoke} from './smoke.mjs';
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
 assert.equal(first.COVENANT_COMPUTE_COORDINATOR_URL,'http://smoke-rpc:14045/coordinator');
});
test('readiness rejects the wrong build or enabled admissions',()=>{
 const health={status:'ok',service:'covenant-compute-x402',protocol:'x402-v2',revision,admissions_enabled:false};verifyHealth(health,revision);
 assert.throws(()=>verifyHealth({...health,admissions_enabled:true},revision));assert.throws(()=>verifyHealth({...health,revision:'unknown'},revision));
});
test('disabled discovery exposes only free empty catalog, without pricing or private fields',()=>{
 const now=Date.now(),manifest={schemaVersion:1,x402Version:2,name:'Covenant Compute',
  description:'Available text inference and its current exact Solana USDC payment terms.',observedAtMs:now,
  available:false,models:[],endpoints:[{path:'/v1/models',method:'GET',price:'free'}]},catalog={object:'list',data:[]};
 verifyClosedDiscovery(manifest,catalog,now,now);
 for(const changed of [{...manifest,available:true},{...manifest,models:[{}]},{...manifest,bridge_secret:'forbidden'},
  {...manifest,observedAtMs:now-5001},{...manifest,endpoints:[...manifest.endpoints,{path:'/v1/chat/completions',method:'POST'}]}]){
  assert.throws(()=>verifyClosedDiscovery(changed,catalog,now,now));
 }
 assert.throws(()=>verifyClosedDiscovery(manifest,{object:'list',data:[{id:'unavailable-model'}]},now,now));
});
test('closed discovery smoke performs only three public GETs and requires no-store',async()=>{
 const calls=[];
 const http=async url=>{
  calls.push(url);
  const body=url.endsWith('/openapi.json')?{openapi:'3.1.0',info:{title:'Covenant Compute'},paths:{}}
   :url.endsWith('/v1/models')?{object:'list',data:[]}:{schemaVersion:1,x402Version:2,name:'Covenant Compute',
   description:'Available text inference and its current exact Solana USDC payment terms.',observedAtMs:Date.now(),
   available:false,models:[],endpoints:[{path:'/v1/models',method:'GET',price:'free'}]};
  return {status:200,body,paymentRequired:false,cacheControl:'no-store'};
 };
 await closedDiscovery(http);
 assert.deepEqual(calls,['http://127.0.0.1:10000/.well-known/x402.json','http://127.0.0.1:10000/v1/models','http://127.0.0.1:10000/openapi.json']);
 await assert.rejects(closedDiscovery(async url=>({...await http(url),cacheControl:null})));
 await assert.rejects(closedDiscovery(async url=>({...await http(url),paymentRequired:true})));
 await assert.rejects(closedDiscovery(async url=>{
  const response=await http(url);
  if(url.endsWith('/openapi.json'))response.body.paths={'/v1/chat/completions':{post:{}}};
  return response;
 }));
});
test('branding smoke verifies official binaries and canonical metadata with only bounded GET/HEAD reads',async()=>{
 const assetRoot=new URL('../../agent-os/crates/covenant-compute-gateway/src/branding/',import.meta.url);
 const definitions=[
  ['/', 'index.html','text/html; charset=utf-8',false],
  ['/favicon.ico','favicon.ico','image/x-icon',false],
  ['/brand/covenant-icon-02026997.png','covenant-icon-02026997.png','image/png',true],
  ['/brand/covenant-banner-4b58cf6c.png','covenant-banner-4b58cf6c.png','image/png',true],
  ['/brand/style.css','style.css','text/css; charset=utf-8',false],
 ];
 const fixtures=new Map(await Promise.all(definitions.map(async([route,file,type,immutable])=>{
  let bytes=await readFile(new URL(file,assetRoot));
  if(route==='/')bytes=Buffer.from(bytes.toString('utf8').replaceAll('{{ORIGIN}}','https://smoke.invalid'));
  return [route,{status:200,bodyBase64:bytes.toString('base64'),contentLength:String(bytes.length),contentType:type,paymentRequired:false,
   cacheControl:immutable?'public, max-age=31536000, immutable':'public, max-age=300',
   contentSecurityPolicy:"default-src 'none'; img-src 'self'; style-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",noSniff:'nosniff',referrerPolicy:'no-referrer'}];
 })));
 const calls=[];
 const http=async(url,options,encoding,maximum)=>{
  assert.equal(new URL(url).origin,'http://127.0.0.1:10000');
  assert.deepEqual(options.headers,{host:'attacker.invalid'});assert.equal(encoding,'base64');
  assert(Number.isSafeInteger(maximum)&&maximum>0&&maximum<=162000);
  assert(['GET','HEAD'].includes(options.method));assert.equal(options.body,undefined);
  const route=new URL(url).pathname;calls.push([route,options.method]);
  const response={...fixtures.get(route)};
  if(options.method==='HEAD')response.bodyBase64='';
  return response;
 };
 await brandingReads(http);
 assert.deepEqual(calls,definitions.flatMap(([route])=>[[route,'GET'],[route,'HEAD']]));
 for(const mutation of [
  (r,route,method)=>{if(route==='/'&&method==='GET')r.bodyBase64=Buffer.from(Buffer.from(r.bodyBase64,'base64').toString().replaceAll('https://smoke.invalid','https://attacker.invalid')).toString('base64');},
  (r,route,method)=>{if(route.includes('icon-')&&method==='GET'){const bytes=Buffer.from(r.bodyBase64,'base64');bytes[0]^=1;r.bodyBase64=bytes.toString('base64');}},
  r=>{r.contentType='application/octet-stream';},r=>{r.cacheControl='no-store';},
  r=>{r.contentSecurityPolicy="default-src *";},r=>{r.noSniff=null;},r=>{r.paymentRequired=true;},
  (r,route,method)=>{if(method==='HEAD')r.bodyBase64=Buffer.from('unexpected body').toString('base64');},
  (r,route,method)=>{if(route==='/'&&method==='GET'){r.bodyBase64=Buffer.alloc(8193).toString('base64');r.contentLength='8193';}},
 ]){
  await assert.rejects(brandingReads(async(url,...args)=>{
   const response=await http(url,...args);mutation(response,new URL(url).pathname,args[0].method);return response;
  }));
 }
});
test('HTTP smoke transport bounds advertised and streamed bodies before parsing',async()=>{
 const json=await readSmokeResponse(new Response('{"ok":true}',{headers:{'content-type':'application/json'}}));
 assert.deepEqual(json.body,{ok:true});
 const binary=await readSmokeResponse(new Response(Uint8Array.of(0,128,255)),'base64',3);
 assert.equal(binary.bodyBase64,'AID/');
 assert.equal((await readSmokeResponse(new Response(null,{headers:{'content-length':'3'}}),'base64',3)).bodyBase64,'');
 await assert.rejects(readSmokeResponse(new Response('x',{headers:{'content-length':'4'}}),'base64',3),/body bound/);
 await assert.rejects(readSmokeResponse(new Response('xxxx'),'base64',3),/body bound/);
 await assert.rejects(readSmokeResponse(new Response('x'),'base64',262145),/bounds/);
 await assert.rejects(readSmokeResponse(new Response('x'),'text',3),/bounds/);
 let cancelled=false;
 const streaming=new ReadableStream({pull(controller){controller.enqueue(new Uint8Array(2));},cancel(){cancelled=true;}});
 await assert.rejects(readSmokeResponse(new Response(streaming),'base64',3),/body bound/);
 assert.equal(cancelled,true);
});
test('branding smoke is an explicit boolean opt-in',async()=>{
 await assert.rejects(smoke(image,revision,revision,false,'true'),/boolean/);
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

test('fixture CLI is reachable by Node fetch on the configured smoke port',{timeout:5000},async t=>{
 const child=spawn(process.execPath,[fileURLToPath(new URL('./smoke-rpc.mjs',import.meta.url))],{env:{...process.env,SMOKE_GENESIS:MAINNET_GENESIS},stdio:['ignore','pipe','pipe']});
 t.after(async()=>{if(child.exitCode===null&&child.signalCode===null){const exited=once(child,'exit');child.kill('SIGTERM');await exited;}});
 const listening=await new Promise((resolve,reject)=>{
  child.once('error',reject);
  child.once('exit',code=>reject(Error(`Fixture exited before readiness (${code})`)));
  child.stdout.once('data',data=>{try{resolve(JSON.parse(data.toString()));}catch(error){reject(error);}});
 });
 const configured=new URL(smokeEnvironment().COVENANT_GATEWAY_SOLANA_RPC);
 assert.equal(listening.status,'listening');assert.equal(String(listening.port),configured.port);
 const response=await fetch(`http://127.0.0.1:${configured.port}/audit`,{signal:AbortSignal.timeout(1000)});
 assert.equal(response.status,200);assert.deepEqual(await response.json(),{genesis:0,refused:0,coordinator:0});
});

test('diagnostics refuse to inspect or log the private app container',async()=>{
 await assert.rejects(fixtureDiagnostics('covenant-gateway-012345abcdef'),/Fixture-only/);
 await assert.rejects(fixtureDiagnostics('arbitrary-container'),/Fixture-only/);
});

test('observer overlay provenance keeps image source separate from accepted binary source',()=>{
 const binary='e'.repeat(40),observer='a'.repeat(40),base='sha256:'+'b'.repeat(64);
 verifyImageProvenance({'org.opencontainers.image.revision':binary},binary);
 const labels={'org.opencontainers.image.revision':observer,'io.covenant.runtime.revision':binary,'io.covenant.observer.revision':observer,'org.opencontainers.image.base.digest':base};
 verifyImageProvenance(labels,binary,observer);
 for(const key of Object.keys(labels)){const bad={...labels};delete bad[key];assert.throws(()=>verifyImageProvenance(bad,binary,observer));}
 assert.throws(()=>verifyImageProvenance({...labels,'io.covenant.runtime.revision':observer},binary,observer));
});
