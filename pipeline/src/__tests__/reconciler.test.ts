import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { IndexerDB } from '../db';
import { reconcile } from '../reconciler';
import { DataKeyVariant } from '../types';

function mockFetcher(entries: any[]): any {
  return {
    fetchStorageEntries: async () => entries,
  };
}

describe('reconcile', () => {
  let db: IndexerDB;

  beforeEach(async () => {
    db = await IndexerDB.create();
  });

  afterEach(() => {
    db.close();
  });

  const contractId = 'CCONTRACT';

  const defaultState = {
    contract_id: contractId,
    admin: null,
    admin_pubkey: null,
    pending_admin: null,
    migration_version: 0,
    is_paused: false,
    total_wrap_count: 0,
    total_revoked: 0,
    storage_bytes: 0,
    slash_threshold: 3,
  };

  it('reports consistent when indexed data matches on-chain state', async () => {
    db.upsertContractState({ ...defaultState, ledger_seq: 100, admin: 'GADMIN' });

    const fetcher = mockFetcher([
      {
        key: { variant: DataKeyVariant.Admin },
        value: { type: 'address', value: 'GADMIN' },
        ledger: 100,
        durability: 'instance',
      },
    ]);

    const report = await reconcile(db, fetcher, contractId);
    expect(report.mismatches).toHaveLength(0);
    expect(report.is_consistent).toBe(true);
  });

  it('catches admin mismatch', async () => {
    db.upsertContractState({ ...defaultState, ledger_seq: 100, admin: 'GADMIN_OLD' });

    const fetcher = mockFetcher([
      {
        key: { variant: DataKeyVariant.Admin },
        value: { type: 'address', value: 'GADMIN_NEW' },
        ledger: 200,
        durability: 'instance',
      },
    ]);

    const report = await reconcile(db, fetcher, contractId);
    expect(report.is_consistent).toBe(false);
    expect(report.mismatches.some((m) => m.startsWith('admin'))).toBe(true);
  });

  it('catches paused mismatch', async () => {
    db.upsertContractState({ ...defaultState, ledger_seq: 100, is_paused: false });

    const fetcher = mockFetcher([
      {
        key: { variant: DataKeyVariant.Admin },
        value: { type: 'address', value: null },
        ledger: 100,
        durability: 'instance',
      },
      {
        key: { variant: DataKeyVariant.Paused },
        value: { type: 'bool', value: true },
        ledger: 500,
        durability: 'instance',
      },
    ]);

    const report = await reconcile(db, fetcher, contractId);
    expect(report.is_consistent).toBe(false);
    expect(report.mismatches.some((m) => m.startsWith('is_paused'))).toBe(true);
  });

  it('handles no indexed state (empty DB) - skips comparison', async () => {
    const fetcher = mockFetcher([
      {
        key: { variant: DataKeyVariant.Admin },
        value: { type: 'address', value: 'GADMIN' },
        ledger: 100,
        durability: 'instance',
      },
    ]);

    const report = await reconcile(db, fetcher, contractId);
    // When no indexed state exists, comparisons are skipped -> no mismatches
    expect(report.mismatches).toHaveLength(0);
    expect(report.is_consistent).toBe(true);
    expect(report.indexed.total_wraps).toBe(0);
  });
});
