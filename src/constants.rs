/// Approximate number of Stellar ledger closes per day (one ledger ≈ 5 seconds).
pub const LEDGERS_PER_DAY: u32 = 17_280;

/// TTL for persistent storage entries (approximately one year).
///
/// This is the threshold (and target) parameter used when extending the
/// live-until ledger of every persistent entry the contract owns.
pub const PERSISTENT_TTL: u32 = LEDGERS_PER_DAY * 365;
