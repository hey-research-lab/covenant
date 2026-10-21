// USDC payments on Base (EIP-3009 transferWithAuthorization), checked here and
// submitted through an x402 facilitator (Coinbase's, or any set by URL). The Rust
// gateway stores the sealed authorization before /evm/settle, and every settle
// reads the chain before asking the facilitator again, so a lost response can
// never move the customer's USDC twice: the authorization's nonce is single use.
import { createHash, createHmac, timingSafeEqual } from 'node:crypto';
import { isDeepStrictEqual } from 'node:util';
import { VerifyError } from '@x402/core/types';
import { canonical, Rejected } from './bridge.mjs';
import { createPricingPolicy, validPaymentAmount } from './pricing.mjs';

// EIP-712 domain values read from each USDC contract (name(), version()).
export const EVM_RAILS = Object.freeze({
  'eip155:8453': { chainId: '0x2105', asset: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913', name: 'USD Coin', version: '2' },
  'eip155:84532': { chainId: '0x14a34', asset: '0x036CbD53842c5426634e7929541eC2318f3dCF7e', name: 'USDC', version: '2' },
});
// The official EVM client signs validBefore = now + maxTimeoutSeconds. Five
// minutes covers the job and its settlement; admission needs two minutes left
// and allows a minute of clock skew past five.
export const EVM_TIMEOUT_SECONDS = 300;
const MIN_REMAINING_SECONDS = 120n;
const MAX_REMAINING_SECONDS = BigInt(EVM_TIMEOUT_SECONDS + 60);
const CONFIRMATIONS = 2n;
// Base makes a block every two seconds, so an authorization is used or expires
// within 180 blocks of verification. The search starts ten blocks before
// verification and stays within a 200-block log query.
const SEARCH_BLOCKS = 199n;
const RESUBMIT_MS = 10_000;
const RECHECK_MS = 30_000;
const AUTHORIZATION_USED = '0x98de503528ee59b575ef0c0a2576a82497bfc029a5685b209e9ec333479b10a5';
const AUTHORIZATION_CANCELED = '0x1cdd46ff242716cdaa72d159d339a485b3438398348d68f09d7c8c0a59353d81';
const TRANSFER = '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef';
const AUTHORIZATION_STATE = '0xe94a0102';
const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
const ZERO_ADDRESS = /^0x0{40}$/;
const NONCE = /^0x[0-9a-fA-F]{64}$/;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const QUANTITY = /^0x(0|[1-9a-f][0-9a-f]*)$/;

const word = hex => hex.toLowerCase().replace(/^0x/, '').padStart(64, '0');
const quantity = value => { if (typeof value !== 'string' || !QUANTITY.test(value)) throw new Error('invalid RPC quantity'); return BigInt(value); };

// The gateway derives the same identifier from the payment header.
export function evmAuthorizationId(from, nonce) {
  return createHash('sha256').update(`eip3009:${from.toLowerCase()}:${nonce.toLowerCase()}`).digest('hex');
}

export function createEvmRail({ config, facilitator, rpc, log = () => {} }) {
  const { network, payTo, resourceUrl, secret } = config;
  const rail = EVM_RAILS[network];
  if (!rail) throw new Error(`Base network must be one of ${Object.keys(EVM_RAILS).join(', ')}`);
  if (config.asset !== rail.asset) throw new Error(`Base asset must be ${rail.asset}, the USDC contract on ${network}`);
  if (!ADDRESS.test(payTo) || ZERO_ADDRESS.test(payTo)) throw new Error('Base receiving address must be a nonzero 0x address');
  if (typeof secret !== 'string' || secret.length < 32) throw new Error('invalid Base rail configuration');
  const pricing = createPricingPolicy(config);
  const now = config.now ?? Date.now;
  function requirements(terms, fresh) {
    const extra = terms?.extra;
    if (terms?.scheme !== 'exact' || terms.network !== network || terms.asset !== rail.asset || terms.payTo !== payTo || !validPaymentAmount(terms.amount) || terms.maxTimeoutSeconds !== EVM_TIMEOUT_SECONDS) throw new Rejected('payment rail or amount mismatch');
    if (fresh && !pricing.allows(terms.amount, now())) throw new Rejected('payment amount is not configured for admission');
    if (!extra || Object.keys(extra).length !== 5 || extra.name !== rail.name || extra.version !== rail.version || !UUID.test(extra.quoteId) || !/^[0-9a-f]{64}$/.test(extra.requestSha256) || !Number.isSafeInteger(extra.expiresAtMs)) throw new Rejected('invalid quote binding');
    if (fresh && (now() > extra.expiresAtMs || extra.expiresAtMs > now() + 90_000)) throw new Rejected('quote expired');
  }
  function authorizationOf(payload, terms) {
    const auth = payload?.payload?.authorization, signature = payload?.payload?.signature;
    if (!auth || typeof signature !== 'string' || !/^0x[0-9a-fA-F]{130,}$/.test(signature)
      || !ADDRESS.test(auth.from) || !NONCE.test(auth.nonce) || typeof auth.to !== 'string' || auth.to.toLowerCase() !== payTo.toLowerCase()
      || auth.value !== terms.amount || !/^[0-9]{1,20}$/.test(auth.validAfter) || !/^[0-9]{1,20}$/.test(auth.validBefore)) throw new Rejected('invalid transfer authorization');
    return auth;
  }
  const seal = value => createHmac('sha256', secret).update(canonical(value)).digest('hex');
  async function assertNetwork() {
    if ((await rpc('eth_chainId', [])) !== rail.chainId) throw new Error('RPC network mismatch');
  }
  // Chain ID and facilitator support, checked before Base is offered and again
  // every RECHECK_MS. A failure answers readiness with 503, so quotes leave
  // Base out; Solana is unaffected. Changes are logged with their reason.
  let check = null, status;
  function prepare() {
    if (check && now() - check.at < RECHECK_MS) return check.result;
    const result = (async () => {
      let chainId, supported;
      try {
        chainId = await rpc('eth_chainId', []);
        supported = await facilitator.getSupported();
      } catch { throw new Error('Base RPC or facilitator unreachable'); }
      if (chainId !== rail.chainId) throw new Error(`Base RPC is not on ${network}`);
      if (!supported?.kinds?.some(kind => kind?.x402Version === 2 && kind.scheme === 'exact' && kind.network === network)) throw new Error(`facilitator does not support ${network}`);
    })();
    check = { at: now(), result };
    result.then(() => null, error => error.message).then(reason => {
      if (reason === status) return;
      status = reason;
      log(reason === null ? `Base payments available on ${network}` : `Base payments unavailable: ${reason}`);
    });
    return result;
  }
  async function blockAt(number) {
    const block = await rpc('eth_getBlockByNumber', [number === undefined ? 'latest' : `0x${number.toString(16)}`, false]);
    return { number: quantity(block?.number), timestamp: quantity(block?.timestamp) };
  }
  // Read at an explicit block, so a lagging node behind a load balancer errors
  // instead of answering for an older block.
  async function used(auth, block) {
    const state = await rpc('eth_call', [{ to: rail.asset, data: AUTHORIZATION_STATE + word(auth.from) + word(auth.nonce) }, `0x${block.toString(16)}`]);
    if (typeof state !== 'string' || !/^0x[0-9a-fA-F]{64}$/.test(state)) throw new Error('invalid authorization state');
    return BigInt(state) !== 0n;
  }
  // How a used nonce was consumed, once CONFIRMATIONS blocks deep: by our
  // transfer (finalized), or by a cancellation or another transfer signed with
  // the same nonce (failed). Null while that is not yet visible.
  async function consumed(auth, terms, fromBlock, head) {
    const from = quantity(fromBlock), to = head < from + SEARCH_BLOCKS ? head : from + SEARCH_BLOCKS;
    const authorizer = `0x${word(auth.from)}`, nonce = `0x${word(auth.nonce)}`;
    const logs = await rpc('eth_getLogs', [{ address: rail.asset, fromBlock, toBlock: `0x${to.toString(16)}`,
      topics: [[AUTHORIZATION_USED, AUTHORIZATION_CANCELED], authorizer, nonce] }]);
    if (!Array.isArray(logs) || logs.length !== 1) return null;
    const hash = logs[0]?.transactionHash;
    if (typeof hash !== 'string' || !/^0x[0-9a-f]{64}$/.test(hash)) return null;
    const receipt = await rpc('eth_getTransactionReceipt', [hash]);
    if (receipt?.transactionHash !== hash || receipt.status !== '0x1' || !Array.isArray(receipt.logs) || head < quantity(receipt.blockNumber) + CONFIRMATIONS) return null;
    const fromAsset = entry => typeof entry?.address === 'string' && entry.address.toLowerCase() === rail.asset.toLowerCase();
    const topics = entry => Array.isArray(entry?.topics) ? entry.topics.map(topic => String(topic).toLowerCase()) : [];
    const index = receipt.logs.findIndex(entry => entry?.logIndex === logs[0].logIndex);
    const [event, ...binding] = topics(receipt.logs[index]);
    if (index < 0 || !fromAsset(receipt.logs[index]) || !isDeepStrictEqual(binding, [authorizer, nonce])) return null;
    if (event === AUTHORIZATION_CANCELED) return { status: 'failed' };
    if (event !== AUTHORIZATION_USED) return null;
    // USDC emits AuthorizationUsed, then the Transfer it authorized. The payer
    // can burn the nonce on a transfer of their own; only ours is a payment.
    const transfer = receipt.logs[index + 1];
    const ours = fromAsset(transfer) && isDeepStrictEqual(topics(transfer), [TRANSFER, authorizer, `0x${word(payTo)}`])
      && typeof transfer.data === 'string' && transfer.data.toLowerCase() === `0x${word(BigInt(terms.amount).toString(16))}`;
    return ours ? { status: 'finalized', transaction_id: hash } : { status: 'failed' };
  }
  // Settle never waits for the facilitator, which holds its reply until the
  // receipt: one submission per authorization at a time, repeated at most
  // every RESUBMIT_MS while the chain shows the authorization unused.
  const submissions = new Map();
  function submit(id, payload, terms) {
    const last = submissions.get(id);
    if (last && (last.running || now() - last.at < RESUBMIT_MS)) return;
    const attempt = { running: true, at: now() };
    submissions.set(id, attempt);
    (async () => {
      try { await facilitator.settle(payload, terms); } catch { /* the next settle reads the chain */ } finally { attempt.running = false; }
    })();
  }
  function unseal(payment, terms) {
    let saved;
    try { saved = JSON.parse(payment?.encoded_transaction); } catch { throw new Rejected('invalid stored authorization'); }
    if (typeof saved?.seal !== 'string' || typeof saved.evidence !== 'object' || saved.evidence === null) throw new Rejected('stored authorization changed');
    const expected = seal(saved.evidence);
    if (saved.seal.length !== expected.length || !timingSafeEqual(Buffer.from(saved.seal), Buffer.from(expected))) throw new Rejected('stored authorization changed');
    const ev = saved.evidence;
    if (!isDeepStrictEqual(ev.terms, terms) || payment.authorization_id !== ev.authorizationId || payment.payer !== ev.payer || payment.transaction_id !== ev.reference) throw new Rejected('stored payment binding changed');
    const auth = authorizationOf(ev.payload, terms);
    if (evmAuthorizationId(auth.from, auth.nonce) !== ev.authorizationId) throw new Rejected('stored payment binding changed');
    return { ev, auth };
  }
  return {
    network,
    async readiness() {
      await prepare();
      const observedAtMs = now();
      if (!pricing.valid || !Number.isSafeInteger(observedAtMs) || observedAtMs < 0) throw new Error('payment pricing unavailable');
      return { schemaVersion: 1, observedAtMs, pricingValid: true, scheme: 'exact',
        network, asset: rail.asset, assetDecimals: 6, payTo, feePayer: '', resourceUrl, ...pricing.snapshot() };
    },
    async verify({ paymentPayload: payload, paymentRequirements: terms, resourceUrl: url }) {
      requirements(terms, true);
      if (url !== resourceUrl || payload?.resource?.url !== resourceUrl || payload?.x402Version !== 2 || !isDeepStrictEqual(payload.accepted, terms)) throw new Rejected('payment payload does not match quote');
      const auth = authorizationOf(payload, terms);
      await assertNetwork();
      const head = await blockAt();
      if (BigInt(auth.validAfter) > head.timestamp) throw new Rejected('authorization not yet valid');
      if (BigInt(auth.validBefore) < head.timestamp + MIN_REMAINING_SECONDS) throw new Rejected('authorization expires too soon');
      if (BigInt(auth.validBefore) > head.timestamp + MAX_REMAINING_SECONDS) throw new Rejected('authorization valid for too long');
      if (await used(auth, head.number)) throw new Rejected('authorization already used');
      let verified;
      try { verified = await facilitator.verify(payload, terms); } catch (error) {
        if (error instanceof VerifyError) throw new Rejected(error.invalidReason ?? 'invalid payment');
        throw error;
      }
      if (!verified?.isValid || typeof verified.payer !== 'string' || verified.payer.toLowerCase() !== auth.from.toLowerCase()) throw new Rejected(verified?.invalidReason ?? 'invalid payment');
      // Nothing is broadcast here. The gateway persists this evidence first.
      const evidence = { payload, authorizationId: evmAuthorizationId(auth.from, auth.nonce), payer: auth.from,
        reference: `${auth.from.toLowerCase()}:${auth.nonce.toLowerCase()}`, terms,
        fromBlock: `0x${(head.number > 10n ? head.number - 10n : 0n).toString(16)}` };
      return { payer: evidence.payer, authorization_id: evidence.authorizationId, transaction_id: evidence.reference,
        encoded_transaction: JSON.stringify({ evidence, seal: seal(evidence) }) };
    },
    async settle({ payment, paymentRequirements: terms }) {
      // Recovery follows the sealed original quote, even after pricing changes.
      requirements(terms, false);
      const { ev, auth } = unseal(payment, terms);
      await assertNetwork();
      const head = await blockAt();
      const validBefore = BigInt(auth.validBefore);
      let outcome = null;
      if (await used(auth, head.number)) outcome = await consumed(auth, terms, ev.fromBlock, head.number);
      // Unused at the head. Once a block CONFIRMATIONS deep has reached
      // validBefore, no block can include it, even after a shallow reorg.
      else if (head.timestamp >= validBefore && (await blockAt(head.number - CONFIRMATIONS)).timestamp >= validBefore) outcome = { status: 'failed' };
      else submit(ev.authorizationId, ev.payload, terms);
      if (outcome) submissions.delete(ev.authorizationId);
      return outcome ?? { status: 'pending' };
    },
  };
}
