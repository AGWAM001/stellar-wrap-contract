import { loadConfig } from './config';
import { IndexerDB } from './db';
import { SorobanFetcher } from './fetcher';
import { processEventBatch, createEmptyState, persistStateToDB } from './processor';
import { backfillEvents } from './backfill';
import { reconcile } from './reconciler';
import type { DerivedState } from './types';

async function main(): Promise<void> {
  const config = loadConfig();
  const db = await IndexerDB.create(config.db_path);
  const fetcher = new SorobanFetcher({
    rpcUrl: config.rpc_url,
    contractId: config.contract_id,
    eventPageSize: config.event_page_size,
  });

  console.log('Soroban-RPC Indexer Pipeline');
  console.log('============================');
  console.log(`Contract: ${config.contract_id}`);
  console.log(`RPC URL:  ${config.rpc_url}`);
  console.log(`DB Path:  ${config.db_path}`);

  // ── Reconcile-only mode ────────────────────────────────────────────
  if (config.reconcile_only) {
    console.log('\nRunning reconciliation...');
    const report = await reconcile(db, fetcher, config.contract_id);
    console.log(`\nReconciliation ${report.is_consistent ? 'PASSED' : 'FAILED'}`);
    if (report.mismatches.length > 0) {
      console.log('Mismatches:');
      for (const m of report.mismatches) {
        console.log(`  - ${m}`);
      }
    }
    console.log(`Indexed wraps: ${report.indexed.total_wraps}`);
    console.log(`On-chain wraps: ${report.onchain.total_wraps}`);
    db.close();
    return;
  }

  // ── Backfill mode ──────────────────────────────────────────────────
  if (config.backfill) {
    console.log('\nStarting historical backfill...');
    await backfillEvents({
      fetcher,
      db,
      contractId: config.contract_id,
      startLedger: config.start_ledger,
      onProgress: (processed, ledger) => {
        if (processed % 500 === 0) {
          console.log(`  Processed ${processed} events (ledger ${ledger})`);
        }
      },
    });
    console.log('Backfill complete.');

    // Run reconciliation after backfill
    console.log('\nRunning post-backfill reconciliation...');
    const report = await reconcile(db, fetcher, config.contract_id);
    console.log(`Reconciliation: ${report.is_consistent ? 'PASSED' : 'FAILED'}`);
    for (const m of report.mismatches) {
      console.log(`  - ${m}`);
    }

    db.close();
    return;
  }

  // ── Live indexing mode ─────────────────────────────────────────────
  console.log('\nStarting live indexing...');

  // Determine starting ledger from cursor or start_ledger config
  const cursor = db.getCursor(`cursor:${config.contract_id}`);
  let startLedger = cursor ? cursor.last_processed_ledger + 1 : config.start_ledger;

  // Initialize state from DB or create empty
  let state: DerivedState;
  const existingState = db.getContractState(config.contract_id);
  if (existingState) {
    state = createEmptyState(config.contract_id, startLedger);
    // Load wraps from DB
    console.log('Loading existing indexed state...');
  } else {
    state = createEmptyState(config.contract_id, startLedger);
  }

  console.log(`Starting from ledger ${startLedger}`);
  console.log(`Polling every ${config.poll_interval_ms}ms\n`);

  // Main polling loop
  let consecutiveErrors = 0;

  while (true) {
    try {
      const { events, latestLedger } = await fetcher.fetchEvents(startLedger);

      if (events.length > 0) {
        const processed = processEventBatch(db, state, events);
        const firstLedger = events[0].ledger;
        const lastLedger = events[events.length - 1].ledger;
        console.log(
          `[${new Date().toISOString()}] Indexed ${processed} events ` +
          `(ledgers ${firstLedger}-${lastLedger}, latest: ${latestLedger})`,
        );
      }

      if (latestLedger > 0) {
        startLedger = latestLedger + 1;
        db.upsertCursor(
          `cursor:${config.contract_id}`,
          config.contract_id,
          latestLedger,
          latestLedger,
        );
      }

      consecutiveErrors = 0;

    } catch (err) {
      consecutiveErrors++;
      console.error(`Error (${consecutiveErrors}):`, err instanceof Error ? err.message : err);

      if (consecutiveErrors >= 10) {
        console.error('Too many consecutive errors. Exiting.');
        break;
      }
    }

    await new Promise((resolve) => setTimeout(resolve, config.poll_interval_ms));
  }

  db.close();
}

process.on('unhandledRejection', (err) => {
  console.error('Unhandled rejection:', err);
  process.exit(1);
});

main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
