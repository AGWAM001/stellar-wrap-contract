#[cfg(test)]
extern crate std;
use soroban_sdk::{contracttype, Address, BytesN, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrapRecord {
    pub timestamp: u64,
    pub data_hash: BytesN<32>,
    pub archetype: Symbol,
    pub period: u64, // Standardized to u64 for better indexing/sorting
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractHealth {
    /// Whether `initialize()` has been called (admin address is set).
    pub initialized: bool,
    /// Whether an admin address is currently configured.
    pub has_admin: bool,
    /// Whether an admin signing (public) key is currently configured.
    pub has_signing_key: bool,
}

/// New struct: FeeParams for algorithmic fee model
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeParams {
    /// base fee in token units
    pub base_fee: i128,
    /// fee increment per scaling step (applied per `scale_step_kib`)
    pub per_kib_fee: i128,
    /// scaling step in KiB (e.g., 1024 means per KiB)
    pub scale_step_kib: u64,
    /// maximum fee cap
    pub max_fee: i128,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Stores the address of the admin.
    Admin,
    /// Stores the Ed25519 public key used to validate backend signatures.
    AdminPubKey,
    /// Stores individual wrap records keyed by user and period.
    Wrap(Address, u64),
    /// Stores the total number of wraps for a specific user.
    WrapCount(Address),
    /// Stores the latest period minted for a specific user.
    LatestPeriod(Address),
    /// Stores the highest storage migration version already applied.
    MigrationVersion,
    /// Stores a list of periods a user has minted wraps for.
    UserPeriods(Address),

    // New instance storage keys for accounting / fee system:
    /// Estimated persistent storage bytes used by this contract (instance-level)
    StorageBytes,
    /// Params for the algorithmic fee function (instance-level)
    FeeParams,
}
