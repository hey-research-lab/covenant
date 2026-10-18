import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { Server } from 'node:http';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ComputeCleanupReport, ComputeConfig, ComputeSessionOptions } from '../src/compute.js';
import type { RunOptions } from '../src/types.js';

const nativeFetch = globalThis.fetch;
const token = 'gpu-test-auth-'.repeat(4);
const policy = { max_usdc_micros: 200000, duration_secs: 300, max_launches: 1 } as const;
const directories: string[] = [];
const servers: Server[] = [];
const readers: ReadableStreamDefaultReader<Uint8Array>[] = [];

const pending = (runKey: string): ComputeCleanupReport => ({
  runKey,
  complete: false,
  settled: [],
  pendingJobIds: ['smoke-job'],
  pendingLaunchKeys: [],
});
const settled = (runKey: string): ComputeCleanupReport => ({
  runKey,
  complete: true,
  pendingJobIds: [],
  pendingLaunchKeys: [],
  settled: [
    {
      job_id: 'smoke-job',
      status: 'cancelled',
      maximum_usdc_micros: 100000,
      receipt: {
        job_id: 'smoke-job',
        runtime_secs: 30,
        charged_usdc_micros: 10000,
        refunded_usdc_micros: 90000,
      },
    },
  ],
});

afterEach(async () => {
  for (const reader of readers.splice(0)) await reader.cancel().catch(() => {});
  for (const server of servers.splice(0)) {
    server.closeAllConnections();
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
  for (const path of [
    '../src/compute.js',
    '../src/readiness.js',
    '../src/backends/index.js',
    '../src/sandbox/e2b.js',
    '../src/run-store.js',
  ])
    vi.doUnmock(path);
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  vi.resetModules();
  for (const directory of directories.splice(0))
    rmSync(directory, { recursive: true, force: true });
});

async function boot(
  options: {
    enabled?: boolean;
    auth?: string;
    isolated?: boolean;
    failTerminalPersist?: boolean;
    recovered?: boolean;
    stored?: unknown[];
  } = {},
) {
  vi.resetModules();
  const directory = mkdtempSync(join(tmpdir(), 'gpu-server-'));
  directories.push(directory);
  const runPath = join(directory, 'runs.json');
  for (const [name, value] of Object.entries({
    NODE_ENV: 'test',
    CODER_AUTH_TOKEN: options.auth ?? token,
    CODER_BACKEND: 'anthropic',
    CODER_MODEL: 'claude-sonnet-4-6',
    ANTHROPIC_API_KEY: 'unused-provider-test-key',
    E2B_API_KEY: options.isolated === false ? '' : 'unused-e2b-test-key',
    COMPUTE_API_TOKEN: 'unused-compute-test-key',
    COMPUTE_API_URL: 'https://compute.invalid',
    COMPUTE_GPU_ENABLED: options.enabled === false ? '0' : '1',
    COMPUTE_DURABLE_STORAGE: '1',
    COMPUTE_JOURNAL_PATH: join(directory, 'gpu.json'),
    RUN_STORE_PATH: runPath,
    LEDGER_PATH: join(directory, 'ledger.json'),
    COMPUTE_MAX_USDC_MICROS: '200000',
    COMPUTE_MAX_DURATION_SECS: '300',
    COMPUTE_MAX_LAUNCHES: '1',
    CODER_IP_MAX_PER_IP: '0',
    E2B_TEMPLATE_ID: '',
  }))
    vi.stubEnv(name, value);
  if (options.stored) writeFileSync(runPath, JSON.stringify(options.stored));

  // No model, sandbox, or compute network can escape this test. Only calls to
  // the local HTTP server below use the native fetch implementation.
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => {
      throw new Error('unexpected external request');
    }),
  );
  const reports = new Map<string, ComputeCleanupReport>();
  const constructions: Array<{ cfg: ComputeConfig; options: ComputeSessionOptions }> = [];
  const launch = vi.fn(async (key: string) => {
    reports.set(key, pending(key));
    return { id: 'smoke-job' };
  });
  const reap = vi.fn(async (key: string) => structuredClone(reports.get(key)!));
  vi.doMock('../src/compute.js', async (importOriginal) => {
    const actual = await importOriginal<typeof import('../src/compute.js')>();
    return {
      ...actual,
      ComputeSession: class {
        readonly key: string;
        constructor(cfg: ComputeConfig, opts: ComputeSessionOptions) {
          constructions.push({ cfg, options: opts });
          this.key = opts.runKey;
          if (!reports.has(this.key))
            reports.set(this.key, {
              runKey: this.key,
              complete: true,
              settled: [],
              pendingJobIds: [],
              pendingLaunchKeys: [],
            });
        }
        launch() {
          return launch(this.key);
        }
        inspect() {
          return structuredClone(reports.get(this.key)!);
        }
        reap() {
          return reap(this.key);
        }
        static async recover() {
          return { runs: [] };
        }
      },
    };
  });
  let ready = true;
  const check = vi.fn(async () => ({ ready, failed: ready ? [] : ['model'], dependencies: {} }));
  vi.doMock('../src/readiness.js', () => ({
    GatewayReadiness: class {
      check = check;
    },
    verifyE2bTariff: vi.fn(async () => {}),
  }));
  const sandbox = {
    exec: vi.fn(async () => ({ stdout: '', stderr: '', exitCode: 0 })),
    readFile: vi.fn(async () => ''),
    writeFile: vi.fn(async () => {}),
    previewUrl: vi.fn(async () => ''),
    destroy: vi.fn(async () => {}),
  };
  const createSandbox = vi.fn(async () => sandbox);
  vi.doMock('../src/sandbox/e2b.js', () => ({
    E2bSandboxProvider: class {
      readonly id = 'e2b';
      create = createSandbox;
      async check() {}
    },
  }));
  const backend = vi.fn(async (opts: RunOptions) => {
    if (opts.computeSession) await opts.computeSession.launch();
    opts.emit({ type: 'message.delta', text: 'working' });
    // Even a legacy backend's premature terminal event must be suppressed.
    opts.emit({ type: 'run.completed', output: 'premature backend success' });
    return {
      output: 'task done',
      usage: { inputTokens: 100, outputTokens: 10, cacheReadTokens: 0, cacheCreationTokens: 0 },
    };
  });
  vi.doMock('../src/backends/index.js', () => ({
    selectBackend: () => ({ id: 'anthropic', run: backend }),
  }));
  if (options.failTerminalPersist) {
    let fail = true;
    vi.doMock('../src/run-store.js', async (importOriginal) => {
      const actual = await importOriginal<typeof import('../src/run-store.js')>();
      return {
        ...actual,
        RunStore: class extends actual.RunStore {
          override save(run: import('../src/run-store.js').StoredRun) {
            if (fail && run.status === 'completed') {
              fail = false;
              throw new Error('injected receipt persistence failure');
            }
            return super.save(run);
          }
        },
      };
    });
  }
  const gateway = await import('../src/server.js');
  servers.push(gateway.server);
  await new Promise<void>((resolve) => gateway.server.listen(0, '127.0.0.1', resolve));
  const address = gateway.server.address();
  if (!address || typeof address === 'string') throw new Error('test gateway did not bind');
  const origin = `http://127.0.0.1:${address.port}`;
  const headers = {
    authorization: `Bearer ${options.auth ?? token}`,
    'content-type': 'application/json',
  };
  const get = (path: string) => nativeFetch(`${origin}${path}`, { headers });
  const post = (body: unknown) =>
    nativeFetch(`${origin}/v1/runs`, { headers, method: 'POST', body: JSON.stringify(body) });
  if (options.recovered !== false) await gateway.recoverCompute();
  return {
    ...gateway,
    origin,
    headers,
    get,
    post,
    reports,
    constructions,
    launch,
    reap,
    backend,
    createSandbox,
    check,
    runPath,
    setReady(value: boolean) {
      ready = value;
    },
  };
}

async function untilStatus(gateway: Awaited<ReturnType<typeof boot>>, id: string, status: string) {
  for (let i = 0; i < 100; i++) {
    const state = (await (await gateway.get(`/v1/runs/${id}`)).json()) as Record<string, any>;
    if (state.status === status) return state;
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  throw new Error(`run did not reach ${status}`);
}

const request = (id: string, gpu = true) => ({
  session_id: id,
  input: 'GPU smoke test',
  max_cost_usd: 0.5,
  ...(gpu ? { gpu: policy } : {}),
});

describe('GPU gateway authorization and lifecycle', () => {
  it('advertises GPU only after recovery, opt-in, and authentication configuration', async () => {
    const gateway = await boot({ recovered: false });
    expect(await (await gateway.get('/v1/capabilities')).json()).toMatchObject({
      features: { gpu_workspace: false },
    });
    await gateway.recoverCompute();
    expect(await (await gateway.get('/v1/capabilities')).json()).toMatchObject({
      features: { gpu_workspace: true },
      compute: { budgetUsd: 0.2, maxLaunches: 1, maxDurationSecs: 300 },
    });
    const unauthenticated = await nativeFetch(`${gateway.origin}/v1/runs`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(request('unauthorized')),
    });
    expect(unauthenticated.status).toBe(401);
    vi.stubEnv('COMPUTE_GPU_ENABLED', '0');
    expect(await (await gateway.get('/v1/capabilities')).json()).toMatchObject({
      features: { gpu_workspace: false },
    });
    expect((await gateway.post(request('disabled'))).status).toBe(503);
    expect(gateway.backend).not.toHaveBeenCalled();
    expect(gateway.launch).not.toHaveBeenCalled();
  });

  it('keeps GPU unavailable with an undersized configured authentication token', async () => {
    const gateway = await boot({ auth: 'short-token' });
    expect(await (await gateway.get('/v1/capabilities')).json()).toMatchObject({
      features: { gpu_workspace: false },
    });
    expect((await gateway.post(request('weak-auth'))).status).toBe(503);
    expect(gateway.backend).not.toHaveBeenCalled();
  });

  it('blocks even ordinary host-shell runs while compute credentials are present', async () => {
    const gateway = await boot({ isolated: false });
    expect(await (await gateway.get('/v1/capabilities')).json()).toMatchObject({
      features: { gpu_workspace: false },
      sandbox: { provider: 'local' },
    });
    const response = await gateway.post(request('host-shell', false));
    expect(response.status).toBe(503);
    expect(await response.json()).toMatchObject({
      error: expect.stringContaining('isolated sandbox'),
    });
    expect(gateway.backend).not.toHaveBeenCalled();
    expect(gateway.launch).not.toHaveBeenCalled();
  });

  it('does not activate a GPU session without an explicit exact dispatch policy', async () => {
    const gateway = await boot();
    const ordinary = await gateway.post(request('ordinary', false));
    expect(ordinary.status).toBe(200);
    const { run_id } = (await ordinary.json()) as { run_id: string };
    await untilStatus(gateway, run_id, 'completed');
    expect(gateway.backend).toHaveBeenCalledTimes(1);
    expect(gateway.backend.mock.calls[0][0].computeSession).toBeUndefined();
    expect(gateway.backend.mock.calls[0][0].computeConfig).toBeUndefined();
    expect(gateway.constructions).toHaveLength(0);
    expect(gateway.launch).not.toHaveBeenCalled();
    expect(
      (await gateway.post({ ...request('too-much'), gpu: { ...policy, max_launches: 2 } })).status,
    ).toBe(400);
    expect(gateway.backend).toHaveBeenCalledTimes(1);
  });

  it('keeps receipt-pending runs stopping, preserves replay during outages, and emits terminal SSE only after settlement', async () => {
    const gateway = await boot();
    const body = request('pending-smoke');
    const submitted = await gateway.post(body);
    expect(submitted.status).toBe(200);
    const { run_id } = (await submitted.json()) as { run_id: string };
    const pendingState = await untilStatus(gateway, run_id, 'stopping');
    expect(pendingState).toMatchObject({
      compute: { complete: false, pendingJobIds: ['smoke-job'] },
    });
    expect(pendingState.costUsd).toBeUndefined();
    expect(gateway.constructions[0].cfg).toMatchObject({
      maxUsdcMicros: 200000,
      maxDurationSecs: 300,
      maxLaunches: 1,
    });
    expect(gateway.constructions[0].options.signal).toBeInstanceOf(AbortSignal);
    expect(gateway.backend.mock.calls[0][0].maxProviderCostUsd).toBeCloseTo(0.18);

    const persistedPending = JSON.parse(readFileSync(gateway.runPath, 'utf8'))[0];
    expect(
      persistedPending.events.some(
        (event: { type: string }) => event.type === 'run.completed' || event.type === 'run.failed',
      ),
    ).toBe(false);
    gateway.setReady(false);
    const replay = await gateway.post(body);
    expect(replay.status).toBe(200);
    expect(await replay.json()).toEqual({ run_id });
    const found = await gateway.get('/v1/run-sessions/pending-smoke');
    expect(found.status).toBe(200);
    expect(await found.json()).toEqual({ run_id });
    expect((await gateway.post(request('new-during-outage'))).status).toBe(503);
    expect(gateway.backend).toHaveBeenCalledTimes(1);
    expect(gateway.launch).toHaveBeenCalledTimes(1);

    const stream = await gateway.get(`/v1/runs/${run_id}/events`);
    const reader = stream.body!.getReader();
    readers.push(reader);
    const first = await reader.read();
    expect(first.done).toBe(false);
    const firstText = new TextDecoder().decode(first.value);
    expect(firstText).toContain('working');
    expect(firstText).not.toContain('run.completed');
    const terminalRead = reader.read();
    gateway.reports.set('pending-smoke', settled('pending-smoke'));
    await gateway.reconcileCompute();
    const terminal = await terminalRead;
    expect(terminal.done).toBe(false);
    expect(new TextDecoder().decode(terminal.value)).toContain('run.completed');
    const final = await untilStatus(gateway, run_id, 'completed');
    expect(final.compute).toEqual(settled('pending-smoke'));
    // 100 input + 10 output tokens at Sonnet prices, $0.12 sandbox ceiling,
    // and the actual $0.01 GPU charge (not the $0.10 reservation or $0.20 cap).
    expect(final.costUsd).toBeCloseTo(0.13045, 6);
    expect(final.output).toContain('"charged_usdc_micros":10000');
    const persistedFinal = JSON.parse(readFileSync(gateway.runPath, 'utf8'))[0];
    expect(
      persistedFinal.events.filter((event: { type: string }) => event.type === 'run.completed'),
    ).toHaveLength(1);
    const budget = (await (await gateway.get('/v1/budget')).json()) as Record<string, any>;
    expect(budget.dailyUsd).toBeCloseTo(final.costUsd, 6);
    expect(budget.reserved).toBe(0);
    expect(budget.active).toBe(0);
  });

  it('does not expose a terminal state or event when final receipt persistence fails', async () => {
    const gateway = await boot({ failTerminalPersist: true });
    const response = await gateway.post(request('persist-failure'));
    const { run_id } = (await response.json()) as { run_id: string };
    await untilStatus(gateway, run_id, 'stopping');
    gateway.reports.set('persist-failure', settled('persist-failure'));
    await gateway.reconcileCompute();
    const state = (await (await gateway.get(`/v1/runs/${run_id}`)).json()) as Record<string, any>;
    expect(state.status).toBe('stopping');
    const stream = await gateway.get(`/v1/runs/${run_id}/events`);
    const reader = stream.body!.getReader();
    readers.push(reader);
    const first = await reader.read();
    expect(new TextDecoder().decode(first.value)).not.toContain('run.completed');
    const terminalRead = reader.read();
    await gateway.reconcileCompute();
    expect(new TextDecoder().decode((await terminalRead).value)).toContain('run.completed');
    await untilStatus(gateway, run_id, 'completed');
    const saved = JSON.parse(readFileSync(gateway.runPath, 'utf8'))[0];
    expect(
      saved.events.filter((event: { type: string }) => event.type === 'run.completed'),
    ).toHaveLength(1);
    expect(saved.output.match(/gpu_workspace final receipts/g)).toHaveLength(1);
  });

  it('keeps startup recovery fail-closed when saved pending evidence has no journal record', async () => {
    const key = 'missing-journal';
    const errorLog = vi.spyOn(console, 'error').mockImplementation(() => {});
    const gateway = await boot({
      stored: [
        {
          id: 'crashed-run',
          sessionId: key,
          requestFingerprint: 'a'.repeat(64),
          reservationId: 'reservation-before-crash',
          reservedMax: 0.5,
          status: 'running',
          events: [],
          gpu: policy,
          compute: pending(key),
          updatedAt: new Date().toISOString(),
        },
      ],
    });
    const state = (await (await gateway.get('/v1/runs/crashed-run')).json()) as Record<string, any>;
    expect(state).toMatchObject({
      status: 'stopping',
      compute: { complete: false, pendingJobIds: ['smoke-job'] },
    });
    expect(await (await gateway.get('/v1/capabilities')).json()).toMatchObject({
      features: { gpu_workspace: false },
    });
    expect((await gateway.post(request('after-crash'))).status).toBe(503);
    const saved = JSON.parse(readFileSync(gateway.runPath, 'utf8'))[0];
    expect(
      saved.events.some(
        (event: { type: string }) => event.type === 'run.completed' || event.type === 'run.failed',
      ),
    ).toBe(false);
    expect(gateway.launch).not.toHaveBeenCalled();
    expect(errorLog).toHaveBeenCalledWith(expect.stringContaining('journal record is missing'));
  });
});
