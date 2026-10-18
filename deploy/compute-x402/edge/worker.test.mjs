import test from 'node:test';
import assert from 'node:assert/strict';
import worker,{createProxy,ORIGIN,PUBLIC_HOST,MAX_BODY_BYTES,MAX_PAYMENT_HEADER_BYTES} from './worker.mjs';
const url=path=>`https://${PUBLIC_HOST}${path}`;
const post=(body='{}',headers={})=>new Request(url('/v1/chat/completions'),{method:'POST',body,headers:{'content-type':'application/json',...headers}});

test('unsupported hostname, path, query, protocol and method cause zero outbound requests',async()=>{
 let calls=0;const proxy=createProxy(async()=>{calls++;throw Error('unexpected');});
 const cases=[new Request('https://attacker.invalid/health'),new Request(url('/admin')),new Request(url('/health?origin=https://attacker.invalid')),new Request('http://'+PUBLIC_HOST+'/health'),new Request(url('/health'),{method:'POST',body:'{}'}),new Request(url('/v1/chat/completions')),new Request(url('/health'),{method:'OPTIONS'}),new Request(url('/v1/chat/completions/'),{method:'POST',body:'{}'})];
 for(const request of cases)assert.equal((await proxy(request)).status,404);assert.equal(calls,0);
});

test('GET and HEAD health use only fixed origin and bypass all caches',async()=>{
 for(const method of ['GET','HEAD']){
  let calls=0;const proxy=createProxy(async(target,init)=>{calls++;assert.equal(target,ORIGIN+'/health');assert.equal(init.method,method);assert.equal(init.body,undefined);assert.equal(init.redirect,'manual');assert.equal(init.cache,'no-store');return new Response(method==='HEAD'?null:'{"status":"ok"}',{headers:{'content-type':'application/json'}});});
  const response=await proxy(new Request(url('/health'),{method}));assert.equal(response.status,200);assert.equal(response.headers.get('cache-control'),'no-store');assert.equal(await response.text(),method==='HEAD'?'':'{"status":"ok"}');assert.equal(calls,1);
 }
 assert.equal(typeof worker.fetch,'function');
});

test('JSON bytes and x402/idempotency headers round trip without parsing or serialization',async()=>{
 const requestBytes=Buffer.from('{ "messages" : [ { "content": "日本語\\nhello", "role": "user" } ], "model":"x", "temperature":0.20 }\n');
 const responseBytes=Uint8Array.from([0,255,31,139,42,10]);let calls=0;
 const proxy=createProxy(async(target,init)=>{
  calls++;assert.equal(target,ORIGIN+'/v1/chat/completions');assert.deepEqual(Buffer.from(init.body),requestBytes);
  assert.equal(init.headers.get('payment-signature'),'signed.standard.payload==');assert.equal(init.headers.get('x-payment'),'legacy.payload==');assert.equal(init.headers.get('idempotency-key'),'stable-id');assert.equal(init.headers.get('content-type'),'application/json');
  for(const header of ['authorization','cookie','host','x-forwarded-host'])assert.equal(init.headers.has(header),false);
  assert.equal(init.headers.get('accept-encoding'),'identity');assert.equal(init.redirect,'manual');assert.equal(init.cache,'no-store');
  return new Response(responseBytes,{status:200,headers:{'payment-response':'settled.standard.payload==','content-type':'application/octet-stream','content-encoding':'gzip','x-receipt':'original'}});
 });
 const response=await proxy(post(requestBytes,{'payment-signature':'signed.standard.payload==','x-payment':'legacy.payload==','idempotency-key':'stable-id',authorization:'secret','cookie':'secret','host':'attacker.invalid','x-forwarded-host':'attacker.invalid'}));
 assert.equal(response.status,200);assert.deepEqual(new Uint8Array(await response.arrayBuffer()),responseBytes);assert.equal(response.headers.get('payment-response'),'settled.standard.payload==');assert.equal(response.headers.get('content-encoding'),'gzip');assert.equal(response.headers.get('x-receipt'),'original');assert.equal(calls,1);
});

test('402 payment requirements and pending/error statuses preserve exact response bytes',async()=>{
 for(const status of [402,202,409,503]){
  const bytes=' {"exact":"body bytes", "n":1.00}\n';const proxy=createProxy(async()=>new Response(bytes,{status,headers:{'payment-required':'original.requirements==','payment-response':'original.response==','content-type':'application/json'}}));
  const response=await proxy(post());assert.equal(response.status,status);assert.equal(await response.text(),bytes);assert.equal(response.headers.get('payment-required'),'original.requirements==');assert.equal(response.headers.get('payment-response'),'original.response==');
 }
});

test('origin redirects are returned once and never followed',async()=>{
 let calls=0;const proxy=createProxy(async(_target,init)=>{calls++;assert.equal(init.redirect,'manual');return new Response('redirect bytes',{status:307,headers:{location:'https://attacker.invalid/capture'}});});
 const response=await proxy(post('{}',{'payment-signature':'private-payment'}));assert.equal(response.status,307);assert.equal(response.headers.get('location'),'https://attacker.invalid/capture');assert.equal(await response.text(),'redirect bytes');assert.equal(calls,1);
});

test('declared, streamed and multibyte oversized bodies are rejected before forwarding',async()=>{
 let calls=0;const proxy=createProxy(async()=>{calls++;return new Response('{}');});
 assert.equal((await proxy(post('{}',{'content-length':String(MAX_BODY_BYTES+1)}))).status,413);
 assert.equal((await proxy(post('€'.repeat(Math.ceil(MAX_BODY_BYTES/3))))).status,413);
 const stream=new ReadableStream({start(controller){controller.enqueue(new Uint8Array(MAX_BODY_BYTES));controller.enqueue(new Uint8Array(1));controller.close();}});
 assert.equal((await proxy(new Request(url('/v1/chat/completions'),{method:'POST',body:stream,duplex:'half'}))).status,413);
 assert.equal((await proxy(post('{}',{'content-length':'invalid'}))).status,400);assert.equal(calls,0);
 const exact=await proxy(post(new Uint8Array(MAX_BODY_BYTES)));assert.equal(exact.status,200);assert.equal(calls,1);
});

test('combined payment headers have a separate 32KiB byte ceiling before forwarding',async()=>{
 let calls=0;const proxy=createProxy(async()=>{calls++;return new Response('{}');});
 assert.equal((await proxy(post('{}',{'payment-signature':'x'.repeat(MAX_PAYMENT_HEADER_BYTES+1)}))).status,431);
 assert.equal((await proxy(post('{}',{'payment-signature':'x'.repeat(MAX_PAYMENT_HEADER_BYTES),'x-payment':'x'}))).status,431);
 assert.equal(calls,0);assert.equal((await proxy(post('{}',{'payment-signature':'x'.repeat(MAX_PAYMENT_HEADER_BYTES)}))).status,200);assert.equal(calls,1);
});

test('uncertain origin exception becomes generic 502 with no retry and no secret reflection',async()=>{
 let calls=0;const proxy=createProxy(async()=>{calls++;throw Error('secret payment bytes or credential URL');});
 const response=await proxy(post('{}',{'payment-signature':'signed-payment'}));assert.equal(response.status,502);assert.deepEqual(await response.json(),{error:{code:'origin_outcome_unknown'}});assert.equal(calls,1);assert.equal(response.headers.get('cache-control'),'no-store');
});

test('interrupted request body never reaches origin; interrupted response stream never retries',async()=>{
 let calls=0;const ingress=createProxy(async()=>{calls++;return new Response('{}');});
 const stream=new ReadableStream({start(controller){controller.error(Error('secret body read error'));}});
 const rejected=await ingress(new Request(url('/v1/chat/completions'),{method:'POST',body:stream,duplex:'half'}));assert.equal(rejected.status,400);assert.equal(calls,0);
 const proxy=createProxy(async()=>{calls++;return new Response(new ReadableStream({start(controller){controller.error(Error('interrupted origin stream'));}}));});
 const response=await proxy(post());await assert.rejects(response.arrayBuffer());assert.equal(calls,1);
});
