import { SorobanFetcher } from './fetcher';
import { IndexerDB } from './db';
import { processEventBatch, createEmptyState } from './processor';
import type { DerivedState } from './types';

export interface BackfillOptions {
  fetcher: SorobanFetcher;
  db: IndexerDB;
  contractId: string;
  startLedger: number;
  endLedger?: number;
  onProgress?: (processed: number, currentLedger: number) => void;
}

/**
 * Backfill historical events for a contract from startLedger to endLedger
 * (or latest ledger if endLedger is not specified).
 *
 * This processes events in pages and incrementally builds the derived state.
 */
export async function backfillEvents(opts: BackfillOptions): Promise<{
  processed: number;
  finalLedger: number;
  state: DerivedState;
}> {
  const { fetcher, db, contractId, startLedger, onProgress } = opts;
  const endLedger = opts.endLedger ?? (await fetcher.getLatestLedger());

  let currentLedger = startLedger;
  let totalProcessed = 0;
  const state = createEmptyState(contractId, startLedger);

  console.log(`Backfilling events from ledger ${startLedger} to ${endLedger}...`);

  while (currentLedger <= endLedger) {
    try {
      const { events, latestLedger } = await fetcher.fetchEvents(currentLedger);

      if (events.length > 0) {
        processEventBatch(db, state, events);
        totalProcessed += events.length;
      }

      const nextLedger = latestLedger > 0 ? latestLedger + 1 : currentLedger + 100;

      if (onProgress) {
        onProgress(totalProcessed, currentLedger);
      }

      // Update cursor
      db.upsertCursor(
        `cursor:${contractId}`,
        contractId,
        latestLedger > 0 ? latestLedger : currentLedger,
        latestLedger > 0 ? latestLedger : currentLedger,
      );

      currentLedger = nextLedger;

    } catch (err) {
      console.error(`Error at ledger ${currentLedger}:`, err);
      // Wait and retry
      await new Promise((resolve) => setTimeout(resolve, 2000));
    }
  }

  console.log(`Backfill complete. Processed ${totalProcessed} events.`);

  return {
    processed: totalProcessed,
    finalLedger: endLedger,
    state,
  };
}
