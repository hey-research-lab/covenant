import { describe, expect, it } from 'vitest';
import {
  runAccounting,
  validAccountingBasis,
  type RunAccountingBasis,
} from '../src/run-accounting.js';

const basis: RunAccountingBasis = {
  sandbox: {
    provider: 'e2b',
    attempted: true,
    maximumUsd: 0.12,
    usdPerSec: 0.0002,
    maximumSeconds: 600,
  },
  model: { method: 'usage_estimate', estimatedUsd: 0.0182604 },
};

describe('persisted run accounting evidence', () => {
  it('uses only the captured tariff and fixed usage fields after serialization/recovery', () => {
    const restored = JSON.parse(JSON.stringify(basis));
    expect(validAccountingBasis(restored)).toBe(true);
    const usage = {
      inputTokens: 5,
      outputTokens: 625,
      cacheReadTokens: 3643,
      cacheCreationTokens: 2074,
      extra: 'https://workspace.example/?token=secret',
    };
    const report = runAccounting(restored, usage, 0.1382604, 0.033951, 0.1722114);
    expect(report.sandbox).toMatchObject({
      maximumUsd: 0.12,
      usdPerSec: 0.0002,
      maximumSeconds: 600,
    });
    expect(report.model.estimatedUsd).toBe(0.0182604);
    expect(report.model.accountedUsd).toBeCloseTo(0.0182604, 8);
    expect(report.totalAccountedUsd).toBe(0.1722114);
    expect(JSON.stringify(report)).not.toContain('secret');
    expect(report.authoritativeBillingReceipt).toBe(false);
  });

  it('keeps legacy breakdown unknown instead of inferring a current sandbox tariff', () => {
    const report = runAccounting(undefined, undefined, 0.1382604, 0.033951, 0.1722114);
    expect(report.model).toEqual({
      method: 'unknown',
      usage: null,
      estimatedUsd: null,
      accountedUsd: null,
    });
    expect(report.sandbox).toEqual({
      provider: 'unknown',
      method: 'unknown',
      maximumUsd: null,
      accountedUsd: null,
      usdPerSec: null,
      maximumSeconds: null,
    });
    expect(report.nonComputeUsd).toBe(0.1382604);
    expect(report.totalAccountedUsd).toBe(0.1722114);
  });

  it('does not call a never-attempted sandbox reservation a charge', () => {
    const beforeCreate = structuredClone(basis);
    beforeCreate.sandbox.attempted = false;
    expect(runAccounting(beforeCreate, undefined, 0, 0, 0).sandbox).toMatchObject({
      maximumUsd: 0.12,
      accountedUsd: 0,
      method: 'not_attempted',
    });
  });

  it.each([
    null,
    {},
    { ...basis, sandbox: { ...basis.sandbox, maximumUsd: -1 } },
    { ...basis, model: { method: 'invoice', estimatedUsd: 0 } },
    { ...basis, model: { method: 'usage_estimate', estimatedUsd: NaN } },
  ])('rejects malformed persisted accounting basis: %j', (value) => {
    expect(validAccountingBasis(value)).toBe(false);
  });
});
