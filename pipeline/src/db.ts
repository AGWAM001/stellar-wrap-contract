import initSqlJs, { SqlJsStatic, Database as SqlJsDatabase } from 'sql.js';
import type {
  EventRow,
  ContractStateRow,
  LedgerCursorRow,
  StorageEntry,
} from './types';

export class IndexerDB {
  private db: SqlJsDatabase;

  constructor(db: SqlJsDatabase) {
    this.db = db;
    this.db.run('PRAGMA foreign_keys = ON');
    this.migrate();
  }

  static async create(dbPath?: string): Promise<IndexerDB> {
    const SQL: SqlJsStatic = await initSqlJs();
    let db: SqlJsDatabase;
    if (dbPath) {
      const fs = await import('fs');
      try {
        const buffer = fs.readFileSync(dbPath);
        db = new SQL.Database(buffer);
      } catch {
        db = new SQL.Database();
      }
    } else {
      db = new SQL.Database();
    }
    return new IndexerDB(db);
  }

  private migrate(): void {
    this.db.run(`
      CREATE TABLE IF NOT EXISTS contract_events (
        id TEXT PRIMARY KEY,
        contract_id TEXT NOT NULL,
        event_type TEXT NOT NULL,
        ledger_seq INTEGER NOT NULL,
        tx_hash TEXT NOT NULL,
        topics_json TEXT NOT NULL,
        data_json TEXT NOT NULL,
        failed_call INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
      )
    `);
    this.db.run(`CREATE INDEX IF NOT EXISTS idx_events_contract_ledger
      ON contract_events(contract_id, ledger_seq)`);
    this.db.run(`CREATE INDEX IF NOT EXISTS idx_events_type
      ON contract_events(event_type)`);

    this.db.run(`
      CREATE TABLE IF NOT EXISTS wrap_records (
        contract_id TEXT NOT NULL,
        user TEXT NOT NULL,
        period INTEGER NOT NULL,
        timestamp INTEGER NOT NULL,
        data_hash TEXT NOT NULL,
        archetype TEXT NOT NULL,
        fsm_state INTEGER NOT NULL,
        fsm_updated_at INTEGER NOT NULL,
        ledger_seq INTEGER NOT NULL,
        tx_hash TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (contract_id, user, period)
      )
    `);
    this.db.run(`CREATE INDEX IF NOT EXISTS idx_wraps_user
      ON wrap_records(contract_id, user)`);
    this.db.run(`CREATE INDEX IF NOT EXISTS idx_wraps_period
      ON wrap_records(contract_id, period)`);

    this.db.run(`
      CREATE TABLE IF NOT EXISTS user_state (
        contract_id TEXT NOT NULL,
        user TEXT NOT NULL,
        wrap_count INTEGER NOT NULL DEFAULT 0,
        latest_period INTEGER,
        alias_hash TEXT,
        slash_count INTEGER NOT NULL DEFAULT 0,
        is_slashed INTEGER NOT NULL DEFAULT 0,
        periods_json TEXT NOT NULL DEFAULT '[]',
        ledger_seq INTEGER NOT NULL,
        updated_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (contract_id, user)
      )
    `);

    this.db.run(`
      CREATE TABLE IF NOT EXISTS contract_state (
        contract_id TEXT PRIMARY KEY,
        admin TEXT,
        admin_pubkey TEXT,
        pending_admin TEXT,
        migration_version INTEGER NOT NULL DEFAULT 0,
        is_paused INTEGER NOT NULL DEFAULT 0,
        total_wrap_count INTEGER NOT NULL DEFAULT 0,
        total_revoked INTEGER NOT NULL DEFAULT 0,
        storage_bytes INTEGER NOT NULL DEFAULT 0,
        slash_threshold INTEGER NOT NULL DEFAULT 3,
        ledger_seq INTEGER NOT NULL,
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
      )
    `);

    this.db.run(`
      CREATE TABLE IF NOT EXISTS storage_snapshots (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        contract_id TEXT NOT NULL,
        ledger_seq INTEGER NOT NULL,
        key_variant TEXT NOT NULL,
        key_json TEXT NOT NULL,
        value_type TEXT NOT NULL,
        value_json TEXT NOT NULL,
        durability TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
      )
    `);
    this.db.run(`CREATE INDEX IF NOT EXISTS idx_snapshots_ledger
      ON storage_snapshots(contract_id, ledger_seq)`);

    this.db.run(`
      CREATE TABLE IF NOT EXISTS ledger_cursor (
        id TEXT PRIMARY KEY,
        contract_id TEXT NOT NULL,
        last_processed_ledger INTEGER NOT NULL DEFAULT 0,
        last_event_ledger INTEGER NOT NULL DEFAULT 0,
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
      )
    `);
  }

  private exec(sql: string, params: unknown[] = []): void {
    this.db.run(sql, params);
  }

  private fetchOne(sql: string, params: unknown[] = []): Record<string, unknown> | null {
    const stmt = this.db.prepare(sql);
    if (params.length > 0) stmt.bind(params);
    let row: Record<string, unknown> | null = null;
    if (stmt.step()) {
      row = stmt.getAsObject();
    }
    stmt.free();
    return row;
  }

  private fetchAll(sql: string, params: unknown[] = []): Record<string, unknown>[] {
    const stmt = this.db.prepare(sql);
    if (params.length > 0) stmt.bind(params);
    const rows: Record<string, unknown>[] = [];
    while (stmt.step()) {
      rows.push(stmt.getAsObject());
    }
    stmt.free();
    return rows;
  }

  // ─── Events ─────────────────────────────────────────────────────────

  insertEvent(event: {
    id: string;
    contract_id: string;
    event_type: string;
    ledger_seq: number;
    tx_hash: string;
    topics_json: string;
    data_json: string;
    failed_call: boolean;
  }): void {
    this.exec(
      `INSERT OR IGNORE INTO contract_events
        (id, contract_id, event_type, ledger_seq, tx_hash, topics_json, data_json, failed_call)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      [event.id, event.contract_id, event.event_type, event.ledger_seq, event.tx_hash, event.topics_json, event.data_json, event.failed_call ? 1 : 0],
    );
  }

  getEventsByLedgerRange(
    contractId: string,
    startLedger: number,
    endLedger: number,
    limit: number = 1000,
  ): EventRow[] {
    return this.fetchAll(
      `SELECT * FROM contract_events
       WHERE contract_id = ? AND ledger_seq >= ? AND ledger_seq <= ?
       ORDER BY ledger_seq ASC
       LIMIT ?`,
      [contractId, startLedger, endLedger, limit],
    ) as unknown as EventRow[];
  }

  getLatestEventLedger(contractId: string): number | null {
    const row = this.fetchOne(
      `SELECT MAX(ledger_seq) as max_ledger FROM contract_events WHERE contract_id = ?`,
      [contractId],
    ) as { max_ledger: number | null } | null;
    return row?.max_ledger ?? null;
  }

  // ─── Wrap Records ───────────────────────────────────────────────────

  upsertWrap(record: {
    contract_id: string;
    user: string;
    period: number;
    timestamp: number;
    data_hash: string;
    archetype: string;
    fsm_state: number;
    fsm_updated_at: number;
    ledger_seq: number;
    tx_hash: string;
  }): void {
    this.exec(
      `INSERT INTO wrap_records
        (contract_id, user, period, timestamp, data_hash, archetype, fsm_state, fsm_updated_at, ledger_seq, tx_hash)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(contract_id, user, period) DO UPDATE SET
        timestamp = excluded.timestamp,
        data_hash = excluded.data_hash,
        archetype = excluded.archetype,
        fsm_state = excluded.fsm_state,
        fsm_updated_at = excluded.fsm_updated_at,
        ledger_seq = excluded.ledger_seq,
        tx_hash = excluded.tx_hash,
        updated_at = datetime('now')`,
      [record.contract_id, record.user, record.period, record.timestamp, record.data_hash, record.archetype, record.fsm_state, record.fsm_updated_at, record.ledger_seq, record.tx_hash],
    );
  }

  removeWrap(contractId: string, user: string, period: number): void {
    this.exec(
      `DELETE FROM wrap_records WHERE contract_id = ? AND user = ? AND period = ?`,
      [contractId, user, period],
    );
  }

  getWrapCount(contractId: string): number {
    const row = this.fetchOne(
      `SELECT COUNT(*) as count FROM wrap_records WHERE contract_id = ?`,
      [contractId],
    ) as { count: number };
    return row.count;
  }

  // ─── User State ─────────────────────────────────────────────────────

  upsertUserState(state: {
    contract_id: string;
    user: string;
    wrap_count: number;
    latest_period: number | null;
    alias_hash: string | null;
    slash_count: number;
    is_slashed: boolean;
    periods: number[];
    ledger_seq: number;
  }): void {
    this.exec(
      `INSERT INTO user_state
        (contract_id, user, wrap_count, latest_period, alias_hash, slash_count, is_slashed, periods_json, ledger_seq)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(contract_id, user) DO UPDATE SET
        wrap_count = excluded.wrap_count,
        latest_period = excluded.latest_period,
        alias_hash = excluded.alias_hash,
        slash_count = excluded.slash_count,
        is_slashed = excluded.is_slashed,
        periods_json = excluded.periods_json,
        ledger_seq = excluded.ledger_seq,
        updated_at = datetime('now')`,
      [state.contract_id, state.user, state.wrap_count, state.latest_period, state.alias_hash, state.slash_count, state.is_slashed ? 1 : 0, JSON.stringify(state.periods), state.ledger_seq],
    );
  }

  // ─── Contract State ─────────────────────────────────────────────────

  upsertContractState(state: {
    contract_id: string;
    admin: string | null;
    admin_pubkey: string | null;
    pending_admin: string | null;
    migration_version: number;
    is_paused: boolean;
    total_wrap_count: number;
    total_revoked: number;
    storage_bytes: number;
    slash_threshold: number;
    ledger_seq: number;
  }): void {
    this.exec(
      `INSERT INTO contract_state
        (contract_id, admin, admin_pubkey, pending_admin, migration_version, is_paused,
         total_wrap_count, total_revoked, storage_bytes, slash_threshold, ledger_seq)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(contract_id) DO UPDATE SET
        admin = excluded.admin,
        admin_pubkey = excluded.admin_pubkey,
        pending_admin = excluded.pending_admin,
        migration_version = excluded.migration_version,
        is_paused = excluded.is_paused,
        total_wrap_count = excluded.total_wrap_count,
        total_revoked = excluded.total_revoked,
        storage_bytes = excluded.storage_bytes,
        slash_threshold = excluded.slash_threshold,
        ledger_seq = excluded.ledger_seq,
        updated_at = datetime('now')`,
      [state.contract_id, state.admin, state.admin_pubkey, state.pending_admin, state.migration_version, state.is_paused ? 1 : 0, state.total_wrap_count, state.total_revoked, state.storage_bytes, state.slash_threshold, state.ledger_seq],
    );
  }

  getContractState(contractId: string): ContractStateRow | null {
    const row = this.fetchOne(
      `SELECT * FROM contract_state WHERE contract_id = ?`,
      [contractId],
    ) as ContractStateRow | null;
    return row ?? null;
  }

  // ─── Storage Snapshots ──────────────────────────────────────────────

  insertStorageSnapshot(entry: StorageEntry, contractId: string): void {
    this.exec(
      `INSERT INTO storage_snapshots
        (contract_id, ledger_seq, key_variant, key_json, value_type, value_json, durability)
       VALUES (?, ?, ?, ?, ?, ?, ?)`,
      [contractId, entry.ledger, String(entry.key.variant), JSON.stringify(entry.key), entry.value.type, JSON.stringify(entry.value), entry.durability],
    );
  }

  // ─── Ledger Cursor ──────────────────────────────────────────────────

  getCursor(id: string): LedgerCursorRow | null {
    const row = this.fetchOne(
      `SELECT * FROM ledger_cursor WHERE id = ?`,
      [id],
    ) as LedgerCursorRow | null;
    return row ?? null;
  }

  upsertCursor(id: string, contractId: string, processedLedger: number, eventLedger: number): void {
    this.exec(
      `INSERT INTO ledger_cursor (id, contract_id, last_processed_ledger, last_event_ledger)
       VALUES (?, ?, ?, ?)
       ON CONFLICT(id) DO UPDATE SET
        last_processed_ledger = excluded.last_processed_ledger,
        last_event_ledger = excluded.last_event_ledger,
        updated_at = datetime('now')`,
      [id, contractId, processedLedger, eventLedger],
    );
  }

  // ─── Stats ──────────────────────────────────────────────────────────

  getStats(contractId: string): Record<string, unknown> {
    const eventCount = this.fetchOne(
      `SELECT COUNT(*) as c FROM contract_events WHERE contract_id = ?`,
      [contractId],
    ) as { c: number };

    const wrapCount = this.fetchOne(
      `SELECT COUNT(*) as c FROM wrap_records WHERE contract_id = ?`,
      [contractId],
    ) as { c: number };

    const userCount = this.fetchOne(
      `SELECT COUNT(*) as c FROM user_state WHERE contract_id = ?`,
      [contractId],
    ) as { c: number };

    const lastLedger = this.fetchOne(
      `SELECT MAX(ledger_seq) as ml FROM contract_events WHERE contract_id = ?`,
      [contractId],
    ) as { ml: number | null };

    return {
      contract_id: contractId,
      total_events: eventCount.c,
      total_wraps: wrapCount.c,
      total_users: userCount.c,
      last_indexed_ledger: lastLedger.ml,
    };
  }

  close(): void {
    this.db.close();
  }
}
