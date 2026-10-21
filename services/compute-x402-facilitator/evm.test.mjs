import test from 'node:test';
import assert from 'node:assert/strict';
import { VerifyError } from '@x402/core/types';
import { createEvmRail, evmAuthorizationId, EVM_RAILS, EVM_TIMEOUT_SECONDS } from './evm.mjs';
import { Rejected } from './bridge.mjs';

const NETWORK = 'eip155:8453';
const PAY_TO = '0x5fA1d0C0bfFE257a20027C523093F941834f5D66';
const PAYER = '0xAbCdEf0000000000000000000000000000000001';
const NONCE = `0x${'0'.repeat(63)}2`;
const TX = `0x${'ab'.repeat(32)}`;
const NOW_S = 1_800_000_000;
const USED = '0x98de503528ee59b575ef0c0a2576a82497bfc029a5685b209e9ec333479b10a5';
const CANCELED = '0x1cdd46ff242716cdaa72d159d339a485b3438398348d68f09d7c8c0a59353d81';
const TRANSFER = '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef';

const hex = n => `0x${n.toString(16)}`;
const topic = value => `0x${value.toLowerCase().replace(/^0x/, '').padStart(64, '0')}`;

function setup(options = {}) {
  const rail = EVM_RAILS[NETWORK];
  const clock = { ms: NOW_S * 1000 };
  const config = { network: NETWORK, asset: rail.asset, payTo: PAY_TO, resourceUrl: 'https://example.com/v1/chat/completions',
    priceAtomic: '1200', secret: 'test-secret-with-at-least-32-characters', now: () => clock.ms, ...options.config };
  const quoteId = '00000000-0000-4000-8000-000000000001';
  const terms = { scheme: 'exact', network: NETWORK, asset: rail.asset, amount: options.amount ?? '1200', payTo: PAY_TO,
    maxTimeoutSeconds: EVM_TIMEOUT_SECONDS, extra: { name: rail.name, version: rail.version, quoteId, requestSha256: 'a'.repeat(64), expiresAtMs: NOW_S * 1000 + 60_000 } };
  // Base: one block every two seconds; the head's timestamp is chain.timestamp.
  const chain = { head: 5000n, timestamp: BigInt(NOW_S), used: false, logs: [], receipts: {}, calls: [] };
  const rpc = async (method, params) => {
    chain.calls.push(method);
    if (method === 'eth_chainId') return chain.chainId ?? rail.chainId;
    if (method === 'eth_getBlockByNumber') {
      const number = params[0] === 'latest' ? chain.head : BigInt(params[0]);
      assert.ok(number <= chain.head);
      return { number: hex(number), timestamp: hex(chain.timestamp - 2n * (chain.head - number)) };
    }
    if (method === 'eth_call') {
      assert.equal(params[0].to, rail.asset);
      assert.match(params[1], /^0x[0-9a-f]+$/, 'state is read at an explicit block');
      return topic(chain.used ? '1' : '0');
    }
    if (method === 'eth_getLogs') {
      const { fromBlock, toBlock } = params[0];
      if (BigInt(toBlock) > chain.head) throw new Error('block range extends beyond current head block');
      assert.ok(BigInt(toBlock) - BigInt(fromBlock) < 200n, 'within a 200-block query');
      chain.logQuery = params[0];
      return chain.logs;
    }
    if (method === 'eth_getTransactionReceipt') return chain.receipts[params[0]] ?? null;
    throw new Error(`unexpected RPC ${method}`);
  };
  const facilitator = { verifies: 0, settles: 0,
    async verify(payload, requirements) {
      this.verifies++; assert.deepEqual(requirements, terms);
      if (options.verifyError) throw new VerifyError(400, { isValid: false, invalidReason: options.verifyError });
      return options.verdict ?? { isValid: true, payer: PAYER };
    },
    settle() {
      this.settles++;
      if (options.settleHangs) return new Promise(() => {});
      return options.settleThrows ? Promise.reject(new Error('lost response')) : Promise.resolve({ success: true, transaction: TX, network: NETWORK, payer: PAYER });
    },
    async getSupported() { return { kinds: options.kinds ?? [{ x402Version: 2, scheme: 'exact', network: NETWORK }], extensions: [], signers: {} }; } };
  const payload = (overrides = {}) => ({ x402Version: 2, resource: { url: config.resourceUrl }, accepted: structuredClone(terms),
    payload: { signature: `0x${'11'.repeat(65)}`, authorization: { from: PAYER, to: PAY_TO, value: terms.amount, validAfter: '0',
      validBefore: String(NOW_S + EVM_TIMEOUT_SECONDS), nonce: NONCE, ...overrides } } });
  const evm = createEvmRail({ config, facilitator, rpc, log: options.log });
  const verify = (overrides) => evm.verify({ paymentPayload: payload(overrides), paymentRequirements: terms, resourceUrl: config.resourceUrl });
  const settle = payment => evm.settle({ payment, paymentRequirements: terms });
  const advance = blocks => { chain.head += BigInt(blocks); chain.timestamp += 2n * BigInt(blocks); };
  // The nonce is consumed in the head block: by our transfer unless told otherwise.
  const consume = ({ event = USED, to = PAY_TO, value = terms.amount } = {}) => {
    const used = { address: rail.asset, logIndex: '0x5', transactionHash: TX, topics: [event, topic(PAYER), topic(NONCE)], data: '0x' };
    const transfer = { address: rail.asset, logIndex: '0x6', transactionHash: TX, topics: [TRANSFER, topic(PAYER), topic(to)], data: topic(BigInt(value).toString(16)) };
    chain.used = true; chain.logs = [used];
    chain.receipts[TX] = { transactionHash: TX, status: '0x1', blockNumber: hex(chain.head), logs: event === USED ? [used, transfer] : [used] };
  };
  return { evm, verify, settle, terms, chain, clock, facilitator, payload, advance, consume, config };
}

test('authorization id is the shared vector the gateway derives', () => {
  assert.equal(evmAuthorizationId(PAYER, NONCE), '4d7d82ffcc60a251090b98c1a1be4dc739b50eab2d384fa083eba828bb25ad8e');
  assert.equal(evmAuthorizationId(PAYER.toUpperCase().replace('0X', '0x'), NONCE), '4d7d82ffcc60a251090b98c1a1be4dc739b50eab2d384fa083eba828bb25ad8e');
});

test('readiness reports the Base rail once the chain and facilitator check out', async () => {
  const f = setup();
  const ready = await f.evm.readiness();
  assert.equal(ready.network, NETWORK);
  assert.equal(ready.asset, EVM_RAILS[NETWORK].asset);
  assert.equal(ready.payTo, PAY_TO);
  assert.equal(ready.feePayer, '');
  assert.equal(ready.legacyPriceAtomic, '1200');
  await f.evm.readiness();
  assert.deepEqual(f.chain.calls, ['eth_chainId'], 'checked once within 30 seconds');
});

test('readiness fails while the facilitator does not support the network or the RPC is on another chain', async () => {
  await assert.rejects(setup({ kinds: [{ x402Version: 2, scheme: 'exact', network: 'eip155:84532' }] }).evm.readiness(), /facilitator does not support eip155:8453/);
  const f = setup(); f.chain.chainId = '0x1';
  await assert.rejects(f.evm.readiness(), /Base RPC is not on eip155:8453/);
});

test('readiness checks again every 30 seconds and logs each change', async () => {
  const logs = [];
  const f = setup({ log: message => logs.push(message) });
  await f.evm.readiness();
  f.chain.chainId = '0x1';
  f.clock.ms += 29_999;
  await f.evm.readiness();
  f.clock.ms += 1;
  await assert.rejects(f.evm.readiness(), /not on eip155:8453/);
  delete f.chain.chainId;
  f.clock.ms += 30_000;
  await f.evm.readiness();
  await new Promise(setImmediate);
  assert.deepEqual(logs, ['Base payments available on eip155:8453', 'Base payments unavailable: Base RPC is not on eip155:8453',
    'Base payments available on eip155:8453']);
});

test('a valid authorization is checked, never submitted, then settles once and finalizes after two blocks', async () => {
  const f = setup();
  const payment = await f.verify();
  assert.equal(payment.payer, PAYER);
  assert.equal(payment.authorization_id, evmAuthorizationId(PAYER, NONCE));
  assert.equal(f.facilitator.verifies, 1);
  assert.equal(f.facilitator.settles, 0);
  assert.deepEqual(await f.settle(payment), { status: 'pending' });
  assert.equal(f.facilitator.settles, 1);
  f.advance(1); f.consume();
  assert.deepEqual(await f.settle(payment), { status: 'pending' }, 'not yet two blocks deep');
  f.advance(2);
  assert.deepEqual(await f.settle(payment), { status: 'finalized', transaction_id: TX });
  assert.equal(f.facilitator.settles, 1, 'a used authorization is never submitted again');
  assert.equal(f.chain.logQuery.fromBlock, hex(4990n));
  assert.equal(f.chain.logQuery.toBlock, hex(f.chain.head), 'the search ends at the head');
});

test('a lost facilitator response is reconciled from the chain', async () => {
  const f = setup({ settleThrows: true });
  const payment = await f.verify();
  assert.deepEqual(await f.settle(payment), { status: 'pending' });
  f.consume(); f.advance(2);
  assert.deepEqual(await f.settle(payment), { status: 'finalized', transaction_id: TX });
  assert.equal(f.facilitator.settles, 1);
});

test('settle never waits for the facilitator and submits again only after ten seconds', async () => {
  let f = setup({ settleHangs: true });
  let payment = await f.verify();
  assert.deepEqual(await f.settle(payment), { status: 'pending' });
  f.clock.ms += 10_000;
  assert.deepEqual(await f.settle(payment), { status: 'pending' });
  assert.equal(f.facilitator.settles, 1, 'one submission at a time');
  f = setup();
  payment = await f.verify();
  await f.settle(payment); await f.settle(payment);
  assert.equal(f.facilitator.settles, 1);
  f.clock.ms += 10_000;
  await f.settle(payment);
  assert.equal(f.facilitator.settles, 2);
});

test('an unused authorization fails once a block two deep has reached validBefore', async () => {
  const f = setup();
  const payment = await f.verify();
  f.advance(EVM_TIMEOUT_SECONDS / 2);
  assert.equal(f.chain.timestamp, BigInt(NOW_S + EVM_TIMEOUT_SECONDS));
  assert.deepEqual(await f.settle(payment), { status: 'pending' }, 'the head alone could still be replaced');
  f.advance(2);
  assert.deepEqual(await f.settle(payment), { status: 'failed' });
});

test('an unused authorization long past validBefore fails without a submission', async () => {
  const f = setup();
  const payment = await f.verify();
  f.advance(200);
  assert.deepEqual(await f.settle(payment), { status: 'failed' });
  assert.equal(f.facilitator.settles, 0);
});

for (const [label, consumption] of [
  ['a transfer to the payer', { to: PAYER }],
  ['a transfer of another amount', { value: '1' }],
  ['a cancellation', { event: CANCELED }],
]) test(`a nonce consumed by ${label} fails and is never counted as payment`, async () => {
  const f = setup();
  const payment = await f.verify();
  f.consume(consumption);
  assert.deepEqual(await f.settle(payment), { status: 'pending' }, 'not yet two blocks deep');
  f.advance(2);
  assert.deepEqual(await f.settle(payment), { status: 'failed' });
  assert.equal(f.facilitator.settles, 0);
});

for (const [label, overrides] of [
  ['another recipient', { to: '0x0000000000000000000000000000000000000001' }],
  ['another amount', { value: '1201' }],
  ['too little time left', { validBefore: String(NOW_S + 119) }],
  ['a validity past six minutes', { validBefore: String(NOW_S + EVM_TIMEOUT_SECONDS + 61) }],
  ['an authorization not yet valid', { validAfter: String(NOW_S + 10) }],
  ['malformed nonce', { nonce: '0x02' }],
]) test(`verification refuses ${label} before asking the facilitator`, async () => {
  const f = setup();
  await assert.rejects(f.verify(overrides), Rejected);
  assert.equal(f.facilitator.verifies, 0);
});

test('verification accepts the widest window the official client signs with a minute of clock skew', async () => {
  const f = setup();
  assert.equal((await f.verify({ validBefore: String(NOW_S + EVM_TIMEOUT_SECONDS + 60) })).payer, PAYER);
});

test('verification refuses a used authorization, a changed quote, a facilitator refusal and another payer', async () => {
  let f = setup(); f.chain.used = true;
  await assert.rejects(f.verify(), /already used/);
  f = setup();
  const changed = f.payload(); changed.accepted.extra.requestSha256 = 'b'.repeat(64);
  await assert.rejects(f.evm.verify({ paymentPayload: changed, paymentRequirements: f.terms, resourceUrl: f.config.resourceUrl }), Rejected);
  f = setup({ verdict: { isValid: false, invalidReason: 'invalid_exact_evm_payload_signature' } });
  await assert.rejects(f.verify(), /invalid_exact_evm_payload_signature/);
  f = setup({ verifyError: 'invalid_exact_evm_insufficient_balance' });
  await assert.rejects(f.verify(), error => error instanceof Rejected && /insufficient_balance/.test(error.message));
  f = setup({ verdict: { isValid: true, payer: '0x0000000000000000000000000000000000000009' } });
  await assert.rejects(f.verify(), Rejected);
});

test('a price outside the configured policy and an RPC on another chain are refused', async () => {
  let f = setup({ amount: '999' });
  await assert.rejects(f.verify(), /not configured/);
  f = setup(); f.chain.chainId = '0x1';
  await assert.rejects(f.verify(), /RPC network mismatch/);
  assert.equal(f.facilitator.verifies, 0);
});

test('tampered or missing stored evidence fails closed', async () => {
  const f = setup();
  const payment = await f.verify();
  const saved = JSON.parse(payment.encoded_transaction);
  saved.evidence.payload.payload.authorization.nonce = `0x${'0'.repeat(63)}3`;
  await assert.rejects(f.settle({ ...payment, encoded_transaction: JSON.stringify(saved) }), /changed/);
  await assert.rejects(f.settle({ ...payment, authorization_id: 'f'.repeat(64) }), /changed/);
  for (const broken of [null, {}, { ...payment, encoded_transaction: 'null' }]) await assert.rejects(f.settle(broken), Rejected);
  assert.equal(f.facilitator.settles, 0);
});

test('configuration must name the official Base USDC and a valid merchant', () => {
  assert.throws(() => setup({ config: { asset: '0x0000000000000000000000000000000000000001' } }));
  assert.throws(() => setup({ config: { payTo: 'not-an-address' } }));
  assert.throws(() => setup({ config: { payTo: `0x${'0'.repeat(40)}` } }));
  assert.throws(() => setup({ config: { network: 'eip155:1' } }));
});
