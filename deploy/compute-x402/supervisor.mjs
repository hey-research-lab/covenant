import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawn} from 'node:child_process';

process.umask(0o077);
const children=[];
let stopping=false;
function stop(code){
  if(stopping)return;
  stopping=true;
  for(const child of children)child.kill('SIGTERM');
  const timeout=setTimeout(()=>{for(const child of children)child.kill('SIGKILL');process.exit(code);},10_000);
  Promise.all(children.map(child=>child.exitCode!==null?Promise.resolve():new Promise(resolve=>child.once('exit',resolve))))
    .then(()=>{clearTimeout(timeout);process.exit(code);});
}
function seed(file,encoded,length){
  const bytes=encoded?Buffer.from(encoded,'base64'):undefined;
  if(bytes&&length!==undefined&&bytes.length!==length)throw Error('Invalid identity seed length');
  if(fs.existsSync(file)){
    const actual=fs.readFileSync(file);
    if((length!==undefined&&actual.length!==length)||(bytes&&(actual.length!==bytes.length||!crypto.timingSafeEqual(actual,bytes))))throw Error('Persistent identity mismatch');
    fs.chmodSync(file,0o600);return;
  }
  if(!bytes)throw Error('A new volume requires explicit identity seeds');
  const fd=fs.openSync(file,'wx',0o600);
  try{fs.writeFileSync(fd,bytes);fs.fsyncSync(fd);}finally{fs.closeSync(fd);}
  const dir=fs.openSync(path.dirname(file),'r');try{fs.fsyncSync(dir);}finally{fs.closeSync(dir);}
}
function start(name,command,args,env){
  const child=spawn(command,args,{cwd:'/opt/covenant',stdio:'inherit',env});
  children.push(child);
  child.once('error',()=>{process.stderr.write(`${name} failed to start\n`);stop(1);});
  child.once('exit',(code)=>{if(!stopping){process.stderr.write(`${name} exited (${code??'signal'})\n`);stop(1);}});
}
async function waitFor(url,headers={}){
  for(let i=0;i<60;i++){
    try{const r=await fetch(url,{headers,signal:AbortSignal.timeout(1000),redirect:'error'});if(r.ok)return;}catch{}
    await new Promise(resolve=>setTimeout(resolve,500));
  }
  throw Error('Private service did not become ready');
}
try{
  const data=process.env.COVENANT_X402_DATA;
  if(!data||!path.isAbsolute(data))throw Error('Persistent directory required');
  if(!['true','false'].includes(process.env.COVENANT_X402_OPERATOR_ENABLED))throw Error('Explicit operator activation required');
  const gateway=path.join(data,'gateway'),operator=path.join(data,'operator'),budget=path.join(data,'budget');
  for(const dir of [gateway,operator,budget,path.join(data,'provider')])fs.mkdirSync(dir,{recursive:true,mode:0o700});
  seed(path.join(gateway,'identity.key'),process.env.COVENANT_GATEWAY_IDENTITY_B64,32);
  seed(path.join(gateway,'fee-payer.json'),process.env.COVENANT_GATEWAY_FEE_PAYER_KEY_B64);
  if(process.env.COVENANT_X402_OPERATOR_ENABLED==='true')seed(path.join(operator,'identity.json'),process.env.COVENANT_COMPUTE_NODE_IDENTITY_B64,32);
  const env={...process.env,
    COVENANT_GATEWAY_HOME:gateway,COVENANT_COMPUTE_NODE_HOME:operator,COVENANT_COMPUTE_BUDGET_HOME:budget,
    COVENANT_GATEWAY_LISTEN:`0.0.0.0:${process.env.PORT??'10000'}`,
    COVENANT_GATEWAY_BRIDGE_URL:'http://127.0.0.1:4043',COVENANT_GATEWAY_BRIDGE_PORT:'4043',
    COVENANT_GATEWAY_FEE_PAYER_KEY_FILE:path.join(gateway,'fee-payer.json'),
    COVENANT_INFERENCE_PROVIDER_PORT:'4044',COVENANT_INFERENCE_PROVIDER_DB_PATH:path.join(data,'provider','calls.sqlite'),
    COVENANT_COMPUTE_OPENAI_URL:'http://127.0.0.1:4044/v1',
    COVENANT_COMPUTE_OPENAI_API_KEY:process.env.COVENANT_INFERENCE_PROVIDER_BEARER_TOKEN};
  delete env.COVENANT_GATEWAY_IDENTITY_B64;delete env.COVENANT_COMPUTE_NODE_IDENTITY_B64;delete env.COVENANT_GATEWAY_FEE_PAYER_KEY_B64;
  for(const signal of ['SIGINT','SIGTERM'])process.on(signal,()=>stop(0));
  start('payment verifier',process.execPath,['services/compute-x402-facilitator/server.mjs'],env);
  // The verifier starts only after its pinned chain has been checked.
  await waitFor('http://127.0.0.1:4043/health',{authorization:`Bearer ${env.COVENANT_GATEWAY_BRIDGE_SECRET}`});
  if(env.COVENANT_X402_OPERATOR_ENABLED==='true'){
    start('inference provider',process.execPath,['services/compute-inference-provider/server.mjs'],env);
    await waitFor('http://127.0.0.1:4044/health');
    start('inference operator','covenant-compute-node',['serve'],env);
  }
  start('inference gateway','covenant-compute-gateway',[],env);
}catch{
  process.stderr.write('Gateway supervisor refused startup; check private configuration and persistent volume.\n');
  stop(1);
}
