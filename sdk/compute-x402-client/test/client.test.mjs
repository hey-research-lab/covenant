import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createPublicKey, verify } from 'node:crypto';
import { AgentKit } from '@coinbase/agentkit';
import { Keypair, VersionedTransaction, TransactionMessage, TransactionInstruction, ComputeBudgetProgram } from '@solana/web3.js';
import bs58 from 'bs58';
import { CovenantX402Client, SOLANA_GENESIS, NETWORK, USDC, agentKitSvmSigner, PAYMENT_COMPUTE_UNITS } from '../index.mjs';
import { covenantInferenceActionProvider, CovenantSolanaKeypairWalletProvider } from '../agentkit.mjs';
import { sha256, stable, strictJson, verifyComputeResult } from '../proof.mjs';
import { Journal } from '../journal.mjs';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { getTransactionDecoder, getTransactionEncoder } from '@solana/kit';
import { x402Client, x402HTTPClient } from '@x402/core/client';
import { ExactSvmScheme } from '@x402/svm/exact/client';

import { fixture, input } from './fixtures/mock-gateway.mjs';

test('installed real AgentKit action uses its Solana wallet and official x402 transfer, verifies receipt and chain bytes',async()=>{
  const f=await fixture(); const provider=covenantInferenceActionProvider(f.config);const kit=await AgentKit.from({walletProvider:f.walletProvider,actionProviders:[provider]});
  try {const actions=kit.getActions();assert.equal(actions.length,1);assert.equal(actions[0].name,'covenant_inference');const request=input();const response=JSON.parse(await actions[0].invoke(request));assert.equal(response.status,'completed');assert.equal(response.amountMicroUsdc,1000);assert.match(response.text,/button/);assert.equal(f.counts().signatures,1);
    const again=JSON.parse(await actions[0].invoke(request));assert.deepEqual(again,response);assert.equal(f.counts().sends,1);
  } finally {provider.close(f.walletProvider);await f.cleanup();}
});

test('restart after unknown paid outcome reuses identical signed payment and never signs twice',async()=>{
  const f=await fixture({dropFirst:true});const request=input();let c=new CovenantX402Client(f.config);
  try{await assert.rejects(c.purchase(request),/authorization_retained/);c.close();c=new CovenantX402Client(f.config);assert.equal((await c.purchase(request)).status,'completed');assert.equal(f.counts().signatures,1);assert.equal(f.counts().quotes,1);assert.equal(f.counts().payments[0],f.counts().payments[1]);}
  finally{c.close();await f.cleanup();}
});

for (const [label, mutate] of [ ['network',q=>q.network='eip155:8453'],['asset',q=>q.asset=Keypair.generate().publicKey.toBase58()],['recipient',q=>q.payTo=Keypair.generate().publicKey.toBase58()],['amount',q=>q.amount='1001'],['digest',q=>q.extra.requestSha256='0'.repeat(64)],['memo',q=>q.extra.memo='other-request'],['expiry',q=>q.extra.expiresAtMs=0] ]) {
  test(`rejects wrong ${label} before any wallet signing or paid dispatch`,async()=>{const f=await fixture({badQuote:mutate});const c=new CovenantX402Client(f.config);try{await assert.rejects(c.purchase(input()));assert.equal(f.counts().signatures,0);assert.equal(f.counts().sends,0);}finally{c.close();await f.cleanup();}});
}

test('changed request cannot reuse idempotency key; durable cumulative cap refuses a third payment',async()=>{
  const f=await fixture();let c=new CovenantX402Client(f.config);const request=input();
  try{await c.purchase(request);await assert.rejects(c.purchase({...request,max_tokens:500}),/idempotency_request_mismatch/);await c.purchase(input());c.close();c=new CovenantX402Client(f.config);await assert.rejects(c.purchase(input()),/cumulative_spend_cap_exceeded/);assert.equal(f.counts().signatures,2);assert.equal(f.counts().sends,2);}
  finally{c.close();await f.cleanup();}
});

test('invalid operator receipt or displayed output never becomes a completed purchase',async()=>{
  const f=await fixture({badResult:r=>r.choices[0].message.content='substituted'});const c=new CovenantX402Client(f.config);
  try{const request=input();await assert.rejects(c.purchase(request),/completion_output_mismatch/);assert.equal(c.journal.get(request.requestId).result,null);assert.ok(c.journal.get(request.requestId).payment);}
  finally{c.close();await f.cleanup();}
});

test('RPC chain mismatch refuses quote and signing',async()=>{const f=await fixture({wrongRpc:true});const c=new CovenantX402Client(f.config);try{await assert.rejects(c.purchase(input()),/rpc_network_mismatch/);assert.equal(f.counts().signatures,0);assert.equal(f.counts().quotes,0);}finally{c.close();await f.cleanup();}});

test('launch message and UTF-8 JSON byte limits reject before quote, reservation, or signing',async()=>{
  const f=await fixture();const c=new CovenantX402Client(f.config);
  try{
    for(const messages of [Array.from({length:33},()=>({role:'user',content:'hi'})),[{role:'user',content:'x'.repeat(4096)}],[{role:'user',content:'界'.repeat(1400)}],[{role:'user',content:'\n'.repeat(2050)}]]){
      const request={...input(),messages};await assert.rejects(c.purchase(request));assert.equal(c.journal.get(request.requestId),undefined);
    }
    assert.equal(f.counts().quotes,0);assert.equal(f.counts().signatures,0);
    const overhead=Buffer.byteLength(JSON.stringify([{role:'user',content:''}]));
    const exact={...input(),messages:[{role:'user',content:'x'.repeat(4096-overhead)}]};
    assert.equal((await c.purchase(exact)).status,'completed');
  }finally{c.close();await f.cleanup();}
});

test('two clients cannot reserve same budget or claim simultaneously',()=>{
  const home=mkdtempSync(join(tmpdir(),'covenant-x402-journal-'));const file=join(home,'journal.sqlite');const a=new Journal(file,'scope',1000),b=new Journal(file,'scope',1000);
  try{const one=a.claim('one','body','digest');assert.throws(()=>b.claim('one','body','digest'),/purchase_in_progress/);a.reserve('one',one.owner,{accepts:[]},1000);const two=b.claim('two','body2','digest2');assert.throws(()=>b.reserve('two',two.owner,{accepts:[]},1),/cumulative_spend_cap_exceeded/);}
  finally{a.close();b.close();rmSync(home,{recursive:true,force:true});}
});

test('parallel Node processes share one durable cumulative ceiling',async()=>{
  const home=mkdtempSync(join(tmpdir(),'covenant-x402-processes-'));const file=join(home,'journal.sqlite');
  const init=new Journal(file,'shared',1000);init.close();
  const code=`import {Journal} from ${JSON.stringify(new URL('../journal.mjs',import.meta.url).href)};
    const j=new Journal(process.argv[1],'shared',1000);try{const r=j.claim(process.argv[2],'body','digest');j.reserve(process.argv[2],r.owner,{},1000);console.log('reserved');}catch(e){if(e.message!=='cumulative_spend_cap_exceeded')throw e;console.log('refused');}finally{j.close();}`;
  try{const run=promisify(execFile);const results=await Promise.all(['a','b'].map(id=>run(process.execPath,['--input-type=module','-e',code,file,id])));assert.deepEqual(results.map(r=>r.stdout.trim()).sort(),['refused','reserved']);}
  finally{rmSync(home,{recursive:true,force:true});}
});

test('duplicate keys are rejected in signed or payment JSON',()=>assert.throws(()=>strictJson('{"amount":"1","amount":"2"}'),/duplicate_json_key/));

test('real Rust BuyerExecutor and SignedWorkReceipt fixture verifies without reserializing floating output',()=>{
  const fixture=strictJson(readFileSync(new URL('./fixtures/native-response.json',import.meta.url),'utf8'));
  const proof=fixture.response.covenant;
  const config={request:fixture.request,requestSha256:sha256(stable(fixture.request)),quoteId:proof.quote_id,gatewayBuyer:proof.envelope.signer_pubkey_b58,operators:[proof.receipt.signer_pubkey_b58]};
  assert.equal(verifyComputeResult(fixture.response,config).text,'Light separates into colors.\nVerified fixture.');
  assert.throws(()=>verifyComputeResult(fixture.response,{...config,request:{...fixture.request,max_tokens:63}}),/token_limit/);
  const tampered=structuredClone(fixture.response);tampered.covenant.output_json=JSON.stringify(tampered.covenant.output);
  assert.notEqual(tampered.covenant.output_json,proof.output_json);
  assert.throws(()=>verifyComputeResult(tampered,config),/output_hash/);
  const badSignature=structuredClone(fixture.response);badSignature.covenant.receipt.signature_b58=bs58.encode(new Uint8Array(64));
  assert.throws(()=>verifyComputeResult(badSignature,config),/invalid_signature/);
});

test('AgentKit official client payment is accepted by the actual gateway facilitator without broadcasting', {skip:!process.env.COVENANT_X402_BRIDGE_PATH},async()=>{
  const f=await fixture({verifyBridge:true});const c=new CovenantX402Client(f.config);
  try{assert.equal((await c.purchase(input())).status,'completed');assert.equal(f.counts().signatures,1);}
  finally{c.close();await f.cleanup();}
});


function unsignedSignerFixture({units=20000,duplicate=false,presigned=false,mutatingWallet=false}={}) {
  const payer=Keypair.generate(),fee=Keypair.generate();let calls=0;
  const instructions=[ComputeBudgetProgram.setComputeUnitLimit({units}),ComputeBudgetProgram.setComputeUnitPrice({microLamports:1}),
    new TransactionInstruction({programId:Keypair.generate().publicKey,keys:[{pubkey:payer.publicKey,isSigner:true,isWritable:false}],data:Buffer.from('unchanged-transfer-and-memo-fixture')})];
  if(duplicate)instructions.push(ComputeBudgetProgram.setComputeUnitLimit({units:20000}));
  const message=new TransactionMessage({payerKey:fee.publicKey,recentBlockhash:fee.publicKey.toBase58(),instructions}).compileToV0Message();
  const transaction=new VersionedTransaction(message);if(presigned)transaction.sign([payer]);
  const kit={...getTransactionDecoder().decode(transaction.serialize()),lifetimeConstraint:{blockhash:fee.publicKey.toBase58(),lastValidBlockHeight:123456789n}};
  const wallet={getNetwork:()=>({protocolFamily:'svm',chainId:SOLANA_GENESIS}),getAddress:()=>payer.publicKey.toBase58(),signTransaction:async(tx)=>{
    calls++;if(mutatingWallet)tx.message.recentBlockhash=Keypair.generate().publicKey.toBase58();tx.sign([payer]);return tx;
  }};
  return {kit,wallet,payer,calls:()=>calls};
}

test('official modifying signer returns the 60k message it signs and preserves every other transaction byte',async()=>{
  const f=unsignedSignerFixture(),before=Buffer.from(getTransactionEncoder().encode(f.kit));
  const [modified]=await agentKitSvmSigner(f.wallet).modifyAndSignTransactions([f.kit]);
  const result=VersionedTransaction.deserialize(getTransactionEncoder().encode(modified));
  assert.equal(Buffer.from(result.message.compiledInstructions[0].data).readUInt32LE(1),60000);
  assert.equal(f.calls(),1);assert.deepEqual(modified.lifetimeConstraint,f.kit.lifetimeConstraint);
  const keyIndex=result.message.staticAccountKeys.findIndex(key=>key.equals(f.payer.publicKey));
  const publicKey=createPublicKey({key:Buffer.concat([Buffer.from('302a300506032b6570032100','hex'),f.payer.publicKey.toBuffer()]),format:'der',type:'spki'});
  assert.ok(verify(null,result.message.serialize(),publicKey,result.signatures[keyIndex]));
  const restored=Buffer.from(result.message.compiledInstructions[0].data);restored.writeUInt32LE(20000,1);result.message.compiledInstructions[0].data=restored;
  assert.deepEqual(Buffer.from(result.message.serialize()),Buffer.from(f.kit.messageBytes));
  assert.deepEqual(Buffer.from(getTransactionEncoder().encode(f.kit)),before,'original transaction remains untouched');
});

test('compute adjustment refuses pre-signed, duplicated or unexpected compute limits before wallet signing',async()=>{
  for(const options of [{units:60000},{units:100000},{duplicate:true},{presigned:true}]){
    const f=unsignedSignerFixture(options);await assert.rejects(agentKitSvmSigner(f.wallet).modifyAndSignTransactions([f.kit]));assert.equal(f.calls(),0);
  }
});

test('wallet message mutation remains forbidden after the explicit compute adjustment',async()=>{
  const f=unsignedSignerFixture({mutatingWallet:true});await assert.rejects(agentKitSvmSigner(f.wallet).modifyAndSignTransactions([f.kit]),/wallet_changed_payment_transaction/);assert.equal(f.calls(),1);
});


test('upgraded client reuses an existing 20k authorization without modifying or signing it again',async()=>{
  const f=await fixture({dropFirst:true,expectedUnits:20000}),request=input();let c=new CovenantX402Client(f.config);
  const legacySigner={address:f.walletProvider.getAddress(),signTransactions:async transactions=>Promise.all(transactions.map(async transaction=>{
    const signed=await f.walletProvider.signTransaction(VersionedTransaction.deserialize(getTransactionEncoder().encode(transaction)));
    const index=signed.message.staticAccountKeys.findIndex(key=>key.toBase58()===f.walletProvider.getAddress());
    return {[f.walletProvider.getAddress()]:signed.signatures[index]};
  }))};
  c.http=new x402HTTPClient(new x402Client().register(NETWORK,new ExactSvmScheme(legacySigner,{rpcUrl:f.config.rpcUrl})));
  try{
    await assert.rejects(c.purchase(request),/authorization_retained/);const before=c.journal.get(request.requestId).payment;c.close();
    c=new CovenantX402Client(f.config);assert.equal((await c.purchase(request)).status,'completed');
    assert.equal(c.journal.get(request.requestId).payment,before);assert.equal(f.counts().payments[0],f.counts().payments[1]);assert.equal(f.counts().signatures,1);assert.equal(f.counts().quotes,1);
  }finally{c.close();await f.cleanup();}
});
