import test from 'node:test';
import assert from 'node:assert/strict';
import { Keypair, VersionedTransaction } from '@solana/web3.js';
import { readFileSync } from 'node:fs';
import { CovenantX402Client } from '../index.mjs';
import { sha256, stable, strictJson, verifyComputeResult } from '../proof.mjs';
import { fixture, input } from './fixtures/mock-gateway.mjs';

// These rates are test data, not a production tariff or usage-metering claim.
const rate = ({ model, max_tokens }) => model === 'small-model' ? 1000 + max_tokens : 2000 + 2 * max_tokens;
const request = (model, max_tokens) => ({ ...input(), model, max_tokens, pricing: 'model_tariff_v1' });

test('real Rust signed tariff fixture verifies the rounded upfront quote and remains replayable after expiry', () => {
  const f = strictJson(readFileSync(new URL('./fixtures/native-tariff-response.json', import.meta.url), 'utf8'));
  const proof = f.response.covenant;
  const verified = verifyComputeResult(f.response, { request: f.request, requestSha256: sha256(stable(f.request)), quoteId: proof.quote_id,
    gatewayBuyer: proof.envelope.signer_pubkey_b58, operators: [proof.receipt.signer_pubkey_b58] });
  assert.equal(verified.job.price_micro_usdc, 165);
  assert.equal(verified.job.capability_requirement.inference_tariff.requested_output_cap, 64);
});

const updateQuote = (job, change) => {
  change(job.input[2].value.inference_tariff);
  job.capability_requirement.inference_tariff = structuredClone(job.input[2].value.inference_tariff);
};
for (const [name, mutate, error] of [
  ['downgrade', j => { j.input.pop(); delete j.capability_requirement.inference_tariff; }, 'inference_tariff_required'],
  ['routing snapshot', j => { j.capability_requirement.inference_tariff.tariff.revision++; }, 'inference_tariff_requirement_mismatch'],
  ['rounding', j => updateQuote(j, q => { q.price_micro_usdc--; j.price_micro_usdc = q.price_micro_usdc; }), 'inference_tariff_price_mismatch'],
  ['model', j => updateQuote(j, q => { q.tariff.model = 'another-model'; }), 'inference_tariff_model_mismatch'],
  ['cap', j => updateQuote(j, q => { q.requested_output_cap++; }), 'inference_tariff_cap_mismatch'],
  ['revision', j => updateQuote(j, q => { q.tariff.revision = 0; }), 'invalid_inference_tariff'],
  ['issue time', j => { j.issued_at_ms = j.input[2].value.inference_tariff.tariff.valid_until_ms; }, 'inference_tariff_issue_time_mismatch'],
  ['unknown fields', j => updateQuote(j, q => { q.actual_tokens = 1; }), 'invalid_inference_tariff_quote'],
]) {
  test(`signed tariff refuses ${name} and keeps the original payment reserved`, async () => {
    const f = await fixture({ badJob: mutate }), client = new CovenantX402Client(f.config), item = request('small-model', 100);
    try {
      await assert.rejects(client.purchase(item), new RegExp(error));
      const saved = client.journal.get(item.requestId);
      assert.ok(saved.payment); assert.equal(saved.result, null); assert.equal(saved.amount, 1000);
    } finally { client.close(); await f.cleanup(); }
  });
}

test('model and requested output cap quotes are charged exactly and share one cumulative ceiling', async () => {
  const f = await fixture({ price: rate });
  const config = { ...f.config, maxPaymentMicroUsdc: 4000, maxTotalMicroUsdc: 5000 };
  let client = new CovenantX402Client(config);
  try {
    const small = await client.purchase(request('small-model', 100));
    const large = await client.purchase(request('large-model', 100));
    assert.equal(small.amountMicroUsdc, 1100);
    assert.equal(large.amountMicroUsdc, 2200);
    client.close(); client = new CovenantX402Client(config);
    assert.equal((await client.purchase(request('small-model', 600))).amountMicroUsdc, 1600);
    await assert.rejects(client.purchase(request('small-model', 1)), /cumulative_spend_cap_exceeded/);
    await assert.rejects(client.purchase(request('large-model', 1001)), /per_payment_cap_exceeded/);
    assert.equal(f.counts().signatures, 3);
    assert.equal(f.counts().sends, 3);
  } finally { client.close(); await f.cleanup(); }
});

test('pending purchase keeps original price, model, cap and signed payment after tariff change and restart', async () => {
  let base = 1000;
  const f = await fixture({ price: body => base + body.max_tokens, pendingFirst: true });
  const config = { ...f.config, maxPaymentMicroUsdc: 10_000, maxTotalMicroUsdc: 20_000 };
  const item = request('small-model', 100);
  let client = new CovenantX402Client(config);
  try {
    assert.equal((await client.purchase(item)).status, 'pending');
    const original = client.journal.get(item.requestId);
    base = 8000; client.close(); client = new CovenantX402Client(config);
    for (const change of [{ model: 'large-model' }, { max_tokens: 101 }, { pricing: undefined }]) {
      await assert.rejects(client.purchase({ ...item, ...change }), /idempotency_request_mismatch/);
    }
    const result = await client.purchase(item);
    assert.equal(result.amountMicroUsdc, 1100);
    const saved = client.journal.get(item.requestId);
    assert.equal(saved.quote, original.quote);
    assert.equal(saved.payment, original.payment);
    assert.equal(saved.amount, original.amount);
    assert.equal(f.counts().quotes, 1);
    assert.equal(f.counts().signatures, 1);
    assert.equal(f.counts().payments[0], f.counts().payments[1]);
    const calls = f.counts().requests;
    assert.equal(JSON.stringify(await client.purchase(item)), JSON.stringify(result));
    assert.equal(f.counts().requests, calls);
    assert.equal((await client.purchase(request('small-model', 100))).amountMicroUsdc, 8100);
  } finally { client.close(); await f.cleanup(); }
});

test('a changed saved reservation cannot dispatch or change the charged quote', async () => {
  const f = await fixture({ pendingFirst: true });
  const client = new CovenantX402Client(f.config), item = input();
  try {
    await client.purchase(item);
    client.journal.db.prepare('UPDATE purchases SET amount=? WHERE id=?').run(999, item.requestId);
    await assert.rejects(client.purchase(item), /reserved_amount_mismatch/);
    assert.equal(f.counts().sends, 1);
    assert.equal(f.counts().signatures, 1);
  } finally { client.close(); await f.cleanup(); }
});

for (const [field, value, error] of [
  ['amountMicroUsdc', 999, 'cached_amount_mismatch'],
  ['text', 'substituted text', 'cached_text_mismatch'],
  ['requestId', 'another-request', 'cached_request_mismatch'],
]) {
  test(`completed replay refuses changed ${field} without any network request`, async () => {
    const f = await fixture(), client = new CovenantX402Client(f.config), item = input();
    try {
      const result = await client.purchase(item); result[field] = value;
      client.journal.db.prepare('UPDATE purchases SET result=? WHERE id=?').run(JSON.stringify(result), item.requestId);
      const calls = f.counts().requests;
      await assert.rejects(client.purchase(item), new RegExp(error));
      assert.equal(f.counts().requests, calls);
    } finally { client.close(); await f.cleanup(); }
  });
}

test('completed replay refuses changed saved accepted amount without another payment', async () => {
  const f = await fixture(), client = new CovenantX402Client(f.config), item = input();
  try {
    await client.purchase(item);
    const row = client.journal.get(item.requestId);
    const payment = JSON.parse(Buffer.from(row.payment, 'base64'));
    payment.accepted.amount = '999';
    client.journal.db.prepare('UPDATE purchases SET payment=? WHERE id=?').run(Buffer.from(JSON.stringify(payment)).toString('base64'), item.requestId);
    const calls = f.counts().requests;
    await assert.rejects(client.purchase(item), /authorization_quote_mismatch/);
    assert.equal(f.counts().requests, calls);
  } finally { client.close(); await f.cleanup(); }
});

for (const pending of [false, true]) {
  test(`${pending ? 'pending' : 'completed'} replay refuses changed payment wire with unchanged accepted terms`, async () => {
    const f = await fixture({ pendingFirst: pending }), client = new CovenantX402Client(f.config), item = input();
    try {
      await client.purchase(item);
      const row = client.journal.get(item.requestId);
      const payment = JSON.parse(Buffer.from(row.payment, 'base64'));
      const tx = VersionedTransaction.deserialize(Buffer.from(payment.payload.transaction, 'base64'));
      const transfer = tx.message.compiledInstructions.find(ix => tx.message.staticAccountKeys[ix.programIdIndex].toBase58() === 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
      const changed = Buffer.from(transfer.data); changed.writeBigUInt64LE(999n, 1); transfer.data = changed;
      payment.payload.transaction = Buffer.from(tx.serialize()).toString('base64');
      client.journal.db.prepare('UPDATE purchases SET payment=? WHERE id=?').run(Buffer.from(JSON.stringify(payment)).toString('base64'), item.requestId);
      await assert.rejects(client.purchase(item), /payment_transaction_not_approved/);
      assert.equal(f.counts().sends, 1); assert.equal(f.counts().signatures, 1);
    } finally { client.close(); await f.cleanup(); }
  });
}

test('legacy request refuses an unexpected signed tariff result', async () => {
  const f = await fixture({ badJob: job => { job.input.push({ type: 'json', value: { inference_tariff: {} } }); } });
  const client = new CovenantX402Client(f.config);
  try { await assert.rejects(client.purchase(input()), /unexpected_inference_tariff/); }
  finally { client.close(); await f.cleanup(); }
});

for (const field of ['payTo', 'gatewayBuyer', 'operators']) {
  test(`completed replay checks current ${field} trust before returning cached proof`, async () => {
    const f = await fixture(), item = input(); let client = new CovenantX402Client(f.config);
    try {
      await client.purchase(item); const calls = f.counts().requests;
      client.close(); const changed = Keypair.generate().publicKey.toBase58();
      client = new CovenantX402Client({ ...f.config, [field]: field === 'operators' ? [changed] : changed });
      await assert.rejects(client.purchase(item));
      assert.equal(f.counts().requests, calls);
    } finally { client.close(); await f.cleanup(); }
  });
}
