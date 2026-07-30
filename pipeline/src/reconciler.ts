import { Address, xdr } from '@stellar/stellar-sdk';
import { SorobanFetcher } from './fetcher';
import { IndexerDB } from './db';
import { decodeDataKey, decodeStorageValue } from './decoder';
import { applyStorageEntryToState, createEmptyState } from './processor';
import type { DerivedState, StorageEntry } from './types';
import { DataKeyVariant } from './types';

export interface ReconciliationReport {
  contract_id: string;
  ledger_seq: number;
  indexed: {
    total_wraps: number;
    total_users: number;
    contract_state_version: number;
  };
  onchain: {
    total_wraps: number;
    contract_state: number;
  };
  mismatches: string[];
  is_consistent: boolean;
}

/**
 * Reconcile indexed state against current on-chain storage.
 * Fetches all current storage entries and compares with the database.
 */
export async function reconcile(
  db: IndexerDB,
  fetcher: SorobanFetcher,
  contractId: string,
): Promise<ReconciliationReport> {
  const mismatches: string[] = [];
  const onChainState = createEmptyState(contractId, 0);

  // Fetch all current storage entries
  const storageEntries = await fetcher.fetchStorageEntries();

  // Build on-chain state from entries
  for (const entry of storageEntries) {
    applyStorageEntryToState(onChainState, entry);
  }

  // Get indexed state from DB
  const indexedState = db.getContractState(contractId);
  const indexedWrapCount = db.getWrapCount(contractId);

  // Compare contract state
  if (indexedState) {
    compareFields('admin', indexedState.admin, onChainState.admin, mismatches);
    compareFields('admin_pubkey', indexedState.admin_pubkey, onChainState.adminPubKey, mismatches);
    compareFields('total_wrap_count', indexedState.total_wrap_count, onChainState.totalWrapCount, mismatches);
    compareFields('total_revoked', indexedState.total_revoked, onChainState.totalRevoked, mismatches);
    compareFields('storage_bytes', indexedState.storage_bytes, onChainState.storageBytes, mismatches);
    compareFields('slash_threshold', indexedState.slash_threshold, onChainState.slashThreshold, mismatches);
    compareFields('is_paused', indexedState.is_paused ? true : false, onChainState.paused, mismatches);
  }

  const isConsistent = mismatches.length === 0;

  return {
    contract_id: contractId,
    ledger_seq: onChainState.ledger_seq,
    indexed: {
      total_wraps: indexedWrapCount,
      total_users: 0,
      contract_state_version: indexedState?.migration_version ?? 0,
    },
    onchain: {
      total_wraps: onChainState.totalWrapCount,
      contract_state: onChainState.migrationVersion,
    },
    mismatches,
    is_consistent: isConsistent,
  };
}

function compareFields(field: string, a: unknown, b: unknown, mismatches: string[]): void {
  const aStr = a != null ? String(a) : '<null>';
  const bStr = b != null ? String(b) : '<null>';
  if (aStr !== bStr) {
    mismatches.push(`${field}: indexed="${aStr}" on-chain="${bStr}"`);
  }
}
