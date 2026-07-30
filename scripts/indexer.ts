import { rpc, xdr, scValToNative } from '@stellar/stellar-sdk';
import * as sqlite3 from 'sqlite3';
import * as path from 'path';

const dbPath = path.resolve(__dirname, 'events.db');
const db = new sqlite3.Database(dbPath);

db.serialize(() => {
  db.run(`
    CREATE TABLE IF NOT EXISTS mint_events (
      id TEXT PRIMARY KEY,
      contract_id TEXT,
      user TEXT,
      period INTEGER,
      archetype TEXT,
      ledger_seq INTEGER,
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
  `);
});

const RPC_URL = process.env.RPC_URL || 'https://soroban-testnet.stellar.org';
const CONTRACT_ID = process.env.CONTRACT_ID || '';
const server = new rpc.Server(RPC_URL);

async function startIndexing() {
  if (!CONTRACT_ID) {
    console.error('Please set CONTRACT_ID environment variable.');
    process.exit(1);
  }

  let lastLedgerStart = 0;
  
  db.get('SELECT MAX(ledger_seq) as maxLedger FROM mint_events', async (err, row: any) => {
    if (row && row.maxLedger) {
      lastLedgerStart = row.maxLedger + 1;
    } else {
      try {
         const latestLedgerResponse = await server.getLatestLedger();
         lastLedgerStart = latestLedgerResponse.sequence - 100;
      } catch (e) {
         console.error('Failed to get latest ledger:', e);
         lastLedgerStart = 0;
      }
    }
    
    console.log(`Starting to index from ledger ${lastLedgerStart}...`);
    pollEvents(lastLedgerStart);
  });
}

async function pollEvents(startLedger: number) {
  let currentStart = startLedger;
  
  while (true) {
    try {
      const getEventsResponse = await server.getEvents({
        startLedger: currentStart,
        filters: [
          {
            type: 'contract',
            contractIds: [CONTRACT_ID],
            topics: [
              [xdr.ScVal.scvSymbol('mint').toXDR('base64')]
            ]
          }
        ],
        limit: 1000
      });
      
      const events = getEventsResponse.events || [];
      if (events.length > 0) {
        console.log(`Found ${events.length} events`);
        for (const event of events) {
           processEvent(event);
        }
      }
      
      if (getEventsResponse.latestLedger) {
        currentStart = getEventsResponse.latestLedger + 1;
      }
      
      await new Promise(resolve => setTimeout(resolve, 5000));
      
    } catch (e) {
      console.error('Error polling events:', e);
      await new Promise(resolve => setTimeout(resolve, 5000));
    }
  }
}

function processEvent(event: rpc.Api.EventResponse) {
  if (event.type !== 'contract') return;
  
  const parsedTopic1 = event.topic[0] as xdr.ScVal;
  if (parsedTopic1.switch().name !== 'scvSymbol' || parsedTopic1.sym().toString() !== 'mint') {
      return;
  }
  
  try {
      const parsedUser = event.topic[1] as xdr.ScVal;
      const user = scValToNative(parsedUser);
      
      const parsedPeriod = event.topic[2] as xdr.ScVal;
      const period = scValToNative(parsedPeriod);
      
      const parsedValue = event.value as xdr.ScVal;
      const archetype = JSON.stringify(scValToNative(parsedValue));
      
      const stmt = db.prepare('INSERT OR IGNORE INTO mint_events (id, contract_id, user, period, archetype, ledger_seq) VALUES (?, ?, ?, ?, ?, ?)');
      stmt.run(event.id, event.contractId, user, period, archetype, event.ledger);
      stmt.finalize();
      
      console.log(`Indexed mint event ${event.id} for user ${user}`);
  } catch (err) {
      console.error('Error parsing event data:', err);
  }
}

startIndexing();
