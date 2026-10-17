#!/usr/bin/env node
// Read-only release acceptance. Never creates jobs, deposits or provider leases.
import assert from 'node:assert/strict';

const [base, revision, coordinator] = process.argv.slice(2);
if (!base || !/^[a-f0-9]{40}$/.test(revision ?? '') || !coordinator) {
  console.error('Usage: node accept-price-history-release.mjs <https-origin> <40-char-revision> <coordinator-public-key>');
  process.exit(2);
}
const origin = new URL(base);
assert.equal(origin.protocol, 'https:', 'production acceptance requires HTTPS');
assert.equal(origin.pathname, '/', 'pass only the public origin');
assert.equal(origin.username + origin.password + origin.search + origin.hash, '', 'origin must not contain credentials or query');
async function get(path) {
  const response = await fetch(new URL(path, origin), {signal: AbortSignal.timeout(20_000), redirect: 'error'});
  assert.equal(response.status, 200, `${path}: HTTP ${response.status}`);
  return response;
}
const health = await (await get('/health')).json();
assert.equal(health.status, 'ok');
assert.equal(health.revision, revision, 'the service must run the exact published revision');
const page = await get('/prices');
assert.match(page.headers.get('content-type') ?? '', /text\/html/);
assert.ok(page.headers.get('content-security-policy'), 'price page requires a content security policy');
assert.match(await page.text(), /GPU price history/i);
const data = await (await get('/prices/history')).json();
assert.equal(data.version, 1);
assert.equal(data.coordinator_pubkey_b58, coordinator, 'persistent coordinator identity changed');
assert.equal(data.unit, 'millionths of the named currency per GPU-hour');
assert.ok(Math.abs(Date.now() - data.generated_at_ms) < 120_000, 'history must be freshly generated');
assert.ok(Array.isArray(data.points));
assert.equal(data.provider_sampling?.persistence_healthy, true, 'provider observations must be durably stored');
assert.equal(data.provider_sampling?.stale, false, 'provider sampling must be fresh');
assert.equal(data.provider_sampling?.state, 'available', 'provider must currently return usable samples');
for (const point of data.points) {
  assert.ok(['USD', 'USDC'].includes(point.currency));
  assert.ok(point.sample_count > 0);
  assert.ok(point.first_observed_at_ms <= point.last_observed_at_ms);
  assert.ok(point.last_observed_at_ms <= data.generated_at_ms);
  for (const name of ['mean_micros_per_gpu_hour', 'min_micros_per_gpu_hour', 'max_micros_per_gpu_hour']) {
    assert.ok(Number.isSafeInteger(point[name]) && point[name] >= 0, name);
  }
  assert.ok(point.min_micros_per_gpu_hour <= point.mean_micros_per_gpu_hour);
  assert.ok(point.mean_micros_per_gpu_hour <= point.max_micros_per_gpu_hour);
  for (const path of point.proof_paths) {
    assert.match(path, /^\/prices\/proofs\/[a-f0-9-]{36}$/);
    const proof = await get(path);
    assert.match(proof.headers.get('content-type') ?? '', /application\/json/);
  }
}
assert.ok(data.points.some(p => p.source === 'provider_market_quote' && p.currency === 'USD'),
  'public release needs at least one real provider sample; an empty page alone is not acceptance');
const malformed = await fetch(new URL('/prices/history?since_ms=2&until_ms=1', origin), {signal: AbortSignal.timeout(20_000)});
assert.equal(malformed.status, 400, 'inverted ranges must fail');
console.log(JSON.stringify({accepted: true, origin: origin.origin, revision, coordinator,
  checked_at: new Date().toISOString(), points: data.points.length,
  provider_samples: data.points.filter(p => p.source === 'provider_market_quote').reduce((n, p) => n + p.sample_count, 0),
  settled_samples: data.points.filter(p => p.currency === 'USDC').reduce((n, p) => n + p.sample_count, 0)}, null, 2));
