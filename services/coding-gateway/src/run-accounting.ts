import type { TokenUsage } from './types.js';

/** Captured before provider work; never reconstruct old tariffs from current config. */
export interface RunAccountingBasis {
  sandbox: {
    provider: 'e2b' | 'local';
    attempted: boolean;
    maximumUsd: number;
    usdPerSec: number;
    maximumSeconds: number;
  };
  model: {
    method: 'unknown' | 'usage_estimate' | 'provider_receipt' | 'no_requests';
    estimatedUsd?: number;
  };
}

export interface RunAccounting {
  version: 1;
  authoritativeBillingReceipt: false;
  model: {
    method: RunAccountingBasis['model']['method'] | 'conservative_hold';
    usage: TokenUsage | null;
    estimatedUsd: number | null;
    accountedUsd: number | null;
  };
  sandbox: {
    provider: 'e2b' | 'local' | 'unknown';
    method: 'maximum_reservation_not_invoice' | 'not_attempted' | 'unknown';
    maximumUsd: number | null;
    accountedUsd: number | null;
    usdPerSec: number | null;
    maximumSeconds: number | null;
  };
  nonComputeUsd: number | null;
  gpuSettledUsd: number | null;
  totalAccountedUsd: number;
  totalMethod: 'settled_components' | 'conservative_run_reservation';
}

const money = (value: unknown): value is number =>
  typeof value === 'number' && Number.isFinite(value) && value >= 0;
const usd = (value: number): number => Number(value.toFixed(9));

export function validAccountingBasis(value: unknown): value is RunAccountingBasis {
  const basis = value as RunAccountingBasis | null;
  return (
    !!basis &&
    typeof basis === 'object' &&
    !!basis.sandbox &&
    ['e2b', 'local'].includes(basis.sandbox.provider) &&
    typeof basis.sandbox.attempted === 'boolean' &&
    money(basis.sandbox.maximumUsd) &&
    money(basis.sandbox.usdPerSec) &&
    money(basis.sandbox.maximumSeconds) &&
    !!basis.model &&
    ['unknown', 'usage_estimate', 'provider_receipt', 'no_requests'].includes(basis.model.method) &&
    (basis.model.estimatedUsd === undefined || money(basis.model.estimatedUsd))
  );
}

function safeUsage(usage: TokenUsage | undefined): TokenUsage | null {
  if (
    !usage ||
    !['inputTokens', 'outputTokens', 'cacheReadTokens', 'cacheCreationTokens'].every(
      (field) =>
        Number.isSafeInteger(usage[field as keyof TokenUsage]) &&
        usage[field as keyof TokenUsage] >= 0,
    )
  )
    return null;
  return {
    inputTokens: usage.inputTokens,
    outputTokens: usage.outputTokens,
    cacheReadTokens: usage.cacheReadTokens,
    cacheCreationTokens: usage.cacheCreationTokens,
  };
}

/** Fixed fields only: no prompts, provider errors, workspace URLs or credentials. */
export function runAccounting(
  basis: RunAccountingBasis | undefined,
  usage: TokenUsage | undefined,
  nonComputeUsd: number | undefined,
  gpuSettledUsd: number | null,
  totalAccountedUsd: number,
): RunAccounting {
  const known = basis && validAccountingBasis(basis) ? basis : undefined;
  const sandboxUsd = known ? (known.sandbox.attempted ? known.sandbox.maximumUsd : 0) : null;
  return {
    version: 1,
    authoritativeBillingReceipt: false,
    model: {
      method: known
        ? known.model.method === 'unknown'
          ? 'conservative_hold'
          : known.model.method
        : 'unknown',
      usage: safeUsage(usage),
      estimatedUsd: known?.model.estimatedUsd === undefined ? null : usd(known.model.estimatedUsd),
      accountedUsd:
        sandboxUsd !== null && nonComputeUsd !== undefined
          ? usd(Math.max(0, nonComputeUsd - sandboxUsd))
          : null,
    },
    sandbox: {
      provider: known?.sandbox.provider ?? 'unknown',
      method: known
        ? known.sandbox.attempted
          ? 'maximum_reservation_not_invoice'
          : 'not_attempted'
        : 'unknown',
      maximumUsd: known ? usd(known.sandbox.maximumUsd) : null,
      accountedUsd: sandboxUsd === null ? null : usd(sandboxUsd),
      usdPerSec: known?.sandbox.usdPerSec ?? null,
      maximumSeconds: known?.sandbox.maximumSeconds ?? null,
    },
    nonComputeUsd: nonComputeUsd === undefined ? null : usd(nonComputeUsd),
    gpuSettledUsd: gpuSettledUsd === null ? null : usd(gpuSettledUsd),
    totalAccountedUsd: usd(totalAccountedUsd),
    totalMethod:
      nonComputeUsd === undefined ? 'conservative_run_reservation' : 'settled_components',
  };
}
