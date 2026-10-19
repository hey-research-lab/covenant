import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {operatorPubkey} from './runtime.mjs';
import {MAINNET_GENESIS} from './smoke-rpc.mjs';
const exec=promisify(execFile),here=path.dirname(fileURLToPath(import.meta.url));
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const IMAGE_PREFIX='ghcr.io/open-covenant/covenant/compute-node-public@sha256:';
export function validateImage(image,revision){
  assert(typeof image==='string'&&image.startsWith(IMAGE_PREFIX)&&/^[a-f0-9]{64}$/.test(image.slice(IMAGE_PREFIX.length)),'Immutable Covenant image digest required');
  assert(/^[a-f0-9]{40}$/.test(revision),'Full expected source revision required');
}
export function verifyImageProvenance(labels,revision,imageRevision=revision){
  assert.equal(labels?.['org.opencontainers.image.revision'],imageRevision,'Unexpected image source');
  if(imageRevision!==revision){
    assert.equal(labels?.['io.covenant.runtime.revision'],revision,'Unexpected preserved runtime source');
    assert.equal(labels?.['io.covenant.observer.revision'],imageRevision,'Unexpected observer source');
    assert(/^sha256:[a-f0-9]{64}$/.test(labels?.['org.opencontainers.image.base.digest']??''),'Pinned base digest required');
  }
}
export function smokeEnvironment(){
  const buyer=crypto.randomBytes(32),fee=crypto.randomBytes(32),merchant=crypto.randomBytes(32),operator=crypto.randomBytes(32);
  const privateKey=crypto.createPrivateKey({key:Buffer.concat([Buffer.from('302e020100300506032b657004220420','hex'),fee]),format:'der',type:'pkcs8'});
  const publicKey=crypto.createPublicKey(privateKey).export({format:'der',type:'spki'}).subarray(-32);
  const keypair=Buffer.from(JSON.stringify([...fee,...publicKey]));
  return {
    PORT:'10000',COVENANT_X402_DATA:'/data',COVENANT_X402_OPERATOR_ENABLED:'false',
    COVENANT_GATEWAY_ENABLED:'true',COVENANT_GATEWAY_ADMISSIONS_ENABLED:'false',COVENANT_INFERENCE_PROVIDER_ENABLED:'false',
    COVENANT_GATEWAY_IDENTITY_B64:buyer.toString('base64'),COVENANT_GATEWAY_FEE_PAYER_KEY_B64:keypair.toString('base64'),
    COVENANT_GATEWAY_BRIDGE_SECRET:crypto.randomBytes(32).toString('hex'),
    COVENANT_GATEWAY_PAY_TO:operatorPubkey(merchant),COVENANT_GATEWAY_FEE_PAYER:operatorPubkey(fee),
    COVENANT_GATEWAY_APPROVED_OPERATOR_PUBKEYS:operatorPubkey(operator),
    COVENANT_COMPUTE_COORDINATOR_URL:'http://smoke-rpc:14045/coordinator',
    COVENANT_GATEWAY_SOLANA_RPC:'http://smoke-rpc:14045/rpc',
    COVENANT_GATEWAY_NETWORK:'solana:5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp',
    COVENANT_GATEWAY_ASSET:'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
    COVENANT_GATEWAY_RESOURCE_URL:'https://smoke.invalid/v1/chat/completions',
    COVENANT_GATEWAY_MODEL:'claude-haiku-4.5',COVENANT_GATEWAY_PRICE_MICRO_USDC:'50000',
    COVENANT_GATEWAY_OFFER_MICRO_USDC:'30000',COVENANT_GATEWAY_RESERVE_MICRO_USDC:'3000000',
    COVENANT_COMPUTE_DAILY_BUDGET_MICRO_USDC:'3000000',COVENANT_GATEWAY_MAX_TOKENS:'2048',COVENANT_GATEWAY_MAX_INPUT_BYTES:'4096',
  };
}
export function verifyHealth(value,revision){
  assert.equal(value.status,'ok');assert.equal(value.service,'covenant-compute-x402');
  assert.equal(value.protocol,'x402-v2');assert.equal(value.revision,revision);assert.equal(value.admissions_enabled,false);
}
export function verifyClosedDiscovery(manifest,catalog,started,ended){
  assert(Number.isSafeInteger(manifest?.observedAtMs)&&manifest.observedAtMs>=started-5000&&manifest.observedAtMs<=ended+5000,'Fresh discovery observation required');
  assert.deepEqual(manifest,{schemaVersion:1,x402Version:2,name:'Covenant Compute',
    description:'Available text inference and its current exact Solana USDC payment terms.',
    observedAtMs:manifest.observedAtMs,available:false,models:[],endpoints:[{path:'/v1/models',method:'GET',price:'free'}]});
  assert.deepEqual(catalog,{object:'list',data:[]});
}
export async function closedDiscovery(http){
  const started=Date.now();
  const manifest=await http('http://127.0.0.1:10000/.well-known/x402.json');
  const catalog=await http('http://127.0.0.1:10000/v1/models');
  const openapi=await http('http://127.0.0.1:10000/openapi.json');
  for(const response of [manifest,catalog,openapi]){
    assert.equal(response.status,200);assert.equal(response.paymentRequired,false);
    assert.equal(response.cacheControl,'no-store');
  }
  verifyClosedDiscovery(manifest.body,catalog.body,started,Date.now());
  assert.equal(openapi.body.openapi,'3.1.0');
  assert.equal(openapi.body.info.title,'Covenant Compute');
  assert.deepEqual(openapi.body.paths,{});
}
const BRANDING_CSP="default-src 'none'; img-src 'self'; style-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
const BRANDING_ROUTES=[
  {path:'/',type:'text/html; charset=utf-8',maximum:8192,cache:'public, max-age=300'},
  {path:'/favicon.ico',type:'image/x-icon',maximum:3000,cache:'public, max-age=300',sha:'2b5136747b64db4eba4bd7f43c41af6c970bdd4680f4d9c0af4854174c9e4d1a'},
  {path:'/brand/covenant-icon-02026997.png',type:'image/png',maximum:56000,cache:'public, max-age=31536000, immutable',sha:'02026997226a417d29b16bd90b19893965217e0ed3d71b80daf7ab1cba10264a'},
  {path:'/brand/covenant-banner-4b58cf6c.png',type:'image/png',maximum:162000,cache:'public, max-age=31536000, immutable',sha:'4b58cf6c68c0179b911953aef8dc1aac872860272de2ac0bd1db3104d7d11d4c'},
  {path:'/brand/style.css',type:'text/css; charset=utf-8',maximum:4096,cache:'public, max-age=300'},
];
export async function brandingReads(http){
  for(const route of BRANDING_ROUTES){
    let length;
    for(const method of ['GET','HEAD']){
      const response=await http(`http://127.0.0.1:10000${route.path}`,{method,headers:{host:'attacker.invalid'}},'base64',route.maximum);
      assert.equal(response.status,200);assert.equal(response.paymentRequired,false);
      assert.equal(response.contentType,route.type);assert.equal(response.cacheControl,route.cache);
      assert.equal(response.contentSecurityPolicy,BRANDING_CSP);assert.equal(response.noSniff,'nosniff');assert.equal(response.referrerPolicy,'no-referrer');
      assert.equal(typeof response.bodyBase64,'string');
      const bytes=Buffer.from(response.bodyBase64,'base64');
      assert.equal(bytes.toString('base64'),response.bodyBase64,'Canonical binary transport required');
      assert(bytes.length<=route.maximum,'Branding response exceeded bound');
      if(method==='HEAD'){
        assert.equal(bytes.length,0);assert.equal(response.contentLength,String(length));continue;
      }
      length=bytes.length;assert(length>0);assert.equal(response.contentLength,String(length));
      if(route.sha)assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'),route.sha,'Official branding asset changed');
      if(route.path==='/'){
        const html=bytes.toString('utf8');
        for(const expected of ['<title>Covenant Compute</title>',
          'rel="canonical" href="https://smoke.invalid/"',
          'property="og:url" content="https://smoke.invalid/"',
          'property="og:image" content="https://smoke.invalid/brand/covenant-banner-4b58cf6c.png"',
          'property="og:image:width" content="3000"','property="og:image:height" content="1000"',
          'rel="icon" href="/brand/covenant-icon-02026997.png" type="image/png" sizes="512x512"',
          'href="/openapi.json"'])assert(html.includes(expected),'Canonical merchant metadata required');
        for(const forbidden of ['attacker.invalid','{{ORIGIN}}','<script','<form'])assert(!html.includes(forbidden),'Unexpected merchant HTML');
      }
    }
  }
}
// Serialized into the isolated image for HTTP checks. Keep it self-contained:
// it uses only platform globals and enforces a bound while consuming the body.
export async function readSmokeResponse(response,encoding='json',maximum=131072){
  if(!['json','base64'].includes(encoding)||!Number.isSafeInteger(maximum)||maximum<1||maximum>262144)throw Error('Invalid smoke response bounds');
  const advertised=response.headers.get('content-length');
  if(advertised!==null&&(!/^\d+$/.test(advertised)||Number(advertised)>maximum))throw Error('Smoke response exceeded body bound');
  const chunks=[];let length=0;
  for await(const chunk of response.body??[]){
    length+=chunk.byteLength;if(length>maximum)throw Error('Smoke response exceeded body bound');chunks.push(chunk);
  }
  const bytes=Buffer.concat(chunks,length);
  return {status:response.status,...(encoding==='json'?{body:JSON.parse(bytes.toString('utf8'))}:{bodyBase64:bytes.toString('base64')}),
    paymentRequired:response.headers.has('payment-required'),cacheControl:response.headers.get('cache-control'),
    contentType:response.headers.get('content-type'),contentLength:advertised,contentSecurityPolicy:response.headers.get('content-security-policy'),
    noSniff:response.headers.get('x-content-type-options'),referrerPolicy:response.headers.get('referrer-policy')};
}
async function docker(args,label){
  try{return (await exec('docker',args,{timeout:180_000,maxBuffer:2*1024*1024})).stdout.trim();}
  catch{throw Error(`Container smoke failed during ${label}`);}
}
function diagnosticText(value){return String(value??'').slice(0,16_384).replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g,'?');}
export async function fixtureDiagnostics(container){
  // This container receives only the public genesis hash, never app secrets.
  assert(/^covenant-rpc-[a-f0-9]{12}$/.test(container),'Fixture-only diagnostics required');
  for(const [stage,args] of [
    ['rpc_fixture_state',['inspect','--format','{{json .State}}',container]],
    ['rpc_fixture_logs',['logs','--tail','80',container]],
  ]){
    try{const result=await exec('docker',args,{timeout:10_000,maxBuffer:32_768});
      const detail=stage==='rpc_fixture_state'?JSON.parse(result.stdout):{stdout:diagnosticText(result.stdout),stderr:diagnosticText(result.stderr)};
      console.error(JSON.stringify({stage,detail}));
    }catch(error){console.error(JSON.stringify({stage,diagnostic_unavailable:true,exit_code:typeof error.code==='number'?error.code:null}));}
  }
}
async function waitForFixture(container){
  const script=`let ready=false,last={};for(let i=0;i<40;i++){try{const r=await fetch('http://127.0.0.1:14045/audit',{signal:AbortSignal.timeout(1000)});last={http_status:r.status};if(r.ok){ready=true;break;}}catch(error){last={error:error.name,code:error.cause?.code};}await new Promise(r=>setTimeout(r,100));}if(!ready){process.stderr.write(JSON.stringify({probe:'rpc_fixture_loopback',...last})+'\\n');process.exit(1);}`;
  try{await exec('docker',['exec',container,'node','--input-type=module','-e',script],{timeout:60_000,maxBuffer:32_768});}
  catch(error){
    console.error(JSON.stringify({stage:'rpc_fixture_probe',exit_code:typeof error.code==='number'?error.code:null,stdout:diagnosticText(error.stdout),stderr:diagnosticText(error.stderr)}));
    throw Error('Container smoke failed during RPC fixture readiness');
  }
}
async function mainnetObservation(){
  const response=await fetch('https://api.mainnet-beta.solana.com',{method:'POST',redirect:'error',signal:AbortSignal.timeout(15_000),headers:{'content-type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:1,method:'getGenesisHash',params:[]})});
  assert(response.ok,'Read-only mainnet genesis RPC unavailable');const result=await response.json();
  assert.equal(result.result,MAINNET_GENESIS,'Unexpected mainnet genesis');return result.result;
}
export async function smoke(image,revision,imageRevision=revision,discovery=false,branding=false){
  assert.equal(typeof discovery,'boolean');assert.equal(typeof branding,'boolean');
  validateImage(image,revision);validateImage(image,imageRevision);process.umask(0o077);
  const genesis=await mainnetObservation(),temporary=await fs.mkdtemp(path.join(os.tmpdir(),'covenant-x402-smoke-'));
  const suffix=crypto.randomBytes(6).toString('hex'),network=`covenant-smoke-${suffix}`,volume=`covenant-smoke-${suffix}`,app=`covenant-gateway-${suffix}`,rpc=`covenant-rpc-${suffix}`;
  const envFile=path.join(temporary,'private.env');
  await fs.writeFile(envFile,Object.entries(smokeEnvironment()).map(([k,v])=>`${k}=${v}`).join('\n')+'\n',{mode:0o600});
  const http=async(url,options={},encoding='json',maximum=131072)=>{
    const script=`const readSmokeResponse=${readSmokeResponse.toString()};const options=${JSON.stringify(options)};const r=await fetch(${JSON.stringify(url)},{...options,headers:{'cache-control':'no-cache',pragma:'no-cache',...options.headers},redirect:'error',cache:'no-store',credentials:'omit',signal:AbortSignal.timeout(3000)});process.stdout.write(JSON.stringify(await readSmokeResponse(r,${JSON.stringify(encoding)},${JSON.stringify(maximum)})));`;
    return JSON.parse(await docker(['exec','--user','node',app,'node','--input-type=module','-e',script],'private HTTP check'));
  };
  const ready=async()=>{
    for(let i=0;i<60;i++){
      try{const result=await http('http://127.0.0.1:10000/health');assert.equal(result.status,200);verifyHealth(result.body,revision);return;}catch{}
      if(await docker(['inspect','--format','{{.State.Running}}',app],'process status')!=='true')throw Error('Container exited before readiness');
      await sleep(500);
    }
    throw Error('Container readiness timed out');
  };
  try{
    await docker(['pull','--quiet',image],'immutable image pull');
    verifyImageProvenance(JSON.parse(await docker(['image','inspect','--format','{{json .Config.Labels}}',image],'image provenance')),revision,imageRevision);
    await docker(['network','create','--internal',network],'isolated network creation');
    await docker(['volume','create',volume],'persistent volume creation');
    await docker(['run','--detach','--name',rpc,'--network',network,'--network-alias','smoke-rpc','--read-only','--cap-drop','ALL','--security-opt','no-new-privileges',
      '--env',`SMOKE_GENESIS=${genesis}`,'--mount',`type=bind,source=${path.join(here,'smoke-rpc.mjs')},target=/smoke/rpc.mjs,readonly`,'--entrypoint','node',image,'/smoke/rpc.mjs'],'read-only RPC fixture');
    await waitForFixture(rpc);
    await docker(['run','--detach','--name',app,'--memory','512m','--network',network,'--mount',`type=volume,source=${volume},target=/data`,'--mount',`type=bind,source=${path.join(here,'smoke-state.mjs')},target=/smoke/state.mjs,readonly`,'--env-file',envFile,image],'actual image entrypoint');
    await ready();
    await docker(['exec','--user','node',app,'node','deploy/compute-x402/operator-cli-check.mjs'],'packaged operator serving command');
    await docker(['exec','--user','node',app,'node','/smoke/state.mjs','record'],'private state recording');
    if(discovery)await closedDiscovery(http);
    if(branding)await brandingReads(http);
    const refused=await http('http://127.0.0.1:10000/v1/chat/completions',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({model:'claude-haiku-4.5',messages:[{role:'user',content:'Never dispatch this smoke test'}],max_tokens:1})});
    assert.equal(refused.status,503);assert.equal(refused.paymentRequired,false);
    await docker(['exec','--user','node',app,'node','/smoke/state.mjs','compare'],'initial read state continuity');
    await docker(['restart','--time','15',app],'persistent volume restart');
    await ready();
    await docker(['exec','--user','node',app,'node','/smoke/state.mjs','compare'],'private state continuity');
    if(discovery){
      await closedDiscovery(http);
    }
    if(branding)await brandingReads(http);
    if(discovery||branding)await docker(['exec','--user','node',app,'node','/smoke/state.mjs','compare'],'restart public reads state continuity');
    const audit=await http('http://smoke-rpc:14045/audit');
    assert.equal(audit.status,200);assert(audit.body.genesis>=2);assert.equal(audit.body.refused,0);assert.equal(audit.body.coordinator,0);
    const result={image,revision,image_revision:imageRevision,mainnet_genesis_checked:true,isolated_network:true,gateway_memory_limit_mib:512,admissions_enabled:false,operator_cli_contract_verified:true,restart_state_preserved:true,closed_discovery_reads_verified:discovery,branding_reads_verified:branding,job_entries:0,paid_dispatches:0};
    if(process.env.GITHUB_STEP_SUMMARY)await fs.appendFile(process.env.GITHUB_STEP_SUMMARY,`\nContainer smoke passed: \`${image}\`\n\nExpected revision \`${revision}\`; 512MiB gateway cap; admissions disabled; persistent keys, seal and journal preserved across restart; zero paid dispatches.\n`);
    return result;
  }catch(error){
    await fixtureDiagnostics(rpc);
    throw error;
  }finally{
    // Never dump app logs, environment, identities or journal contents.
    for(const name of [app,rpc])await docker(['rm','--force',name],'cleanup').catch(()=>{});
    await docker(['volume','rm','--force',volume],'volume cleanup').catch(()=>{});
    await docker(['network','rm',network],'network cleanup').catch(()=>{});
    await fs.rm(temporary,{recursive:true,force:true});
  }
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
  try{
    assert([undefined,'true','false'].includes(process.env.SMOKE_DISCOVERY),'SMOKE_DISCOVERY must be true or false');
    assert([undefined,'true','false'].includes(process.env.SMOKE_BRANDING),'SMOKE_BRANDING must be true or false');
    console.log(JSON.stringify(await smoke(process.env.SMOKE_IMAGE,process.env.SMOKE_REVISION,process.env.SMOKE_IMAGE_REVISION??process.env.SMOKE_REVISION,process.env.SMOKE_DISCOVERY==='true',process.env.SMOKE_BRANDING==='true')));
  }
  catch(error){process.stderr.write(`${error instanceof assert.AssertionError?'Container smoke assertion failed':error.message}\n`);process.exitCode=1;}
}
