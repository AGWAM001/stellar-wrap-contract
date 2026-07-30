import * as dotenv from 'dotenv';
import * as path from 'path';
import type { IndexerConfig } from './types';

dotenv.config({ path: path.resolve(__dirname, '../.env') });

export function loadConfig(): IndexerConfig {
  const contractId = process.env.CONTRACT_ID || '';
  if (!contractId) {
    throw new Error(
      'CONTRACT_ID environment variable is required. ' +
      'Set it in pipeline/.env or export CONTRACT_ID=...'
    );
  }

  return {
    rpc_url: process.env.RPC_URL || 'https://soroban-testnet.stellar.org',
    contract_id: contractId,
    db_path: process.env.DB_PATH || path.resolve(__dirname, '../indexer.db'),
    poll_interval_ms: parseInt(process.env.POLL_INTERVAL_MS || '5000', 10),
    event_page_size: parseInt(process.env.EVENT_PAGE_SIZE || '100', 10),
    start_ledger: parseInt(process.env.START_LEDGER || '1', 10),
    backfill: process.argv.includes('--backfill'),
    reconcile_only: process.argv.includes('--reconcile-only'),
  };
}
