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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferFeeConfig {
    /// Amount of `token` charged to the sender for each successful transfer.
    pub amount: i128,
    /// Address that receives transfer fees.
    pub recipient: Address,
    /// Soroban token contract used to collect fees.
    pub token: Address,
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
    /// Stores the periods currently owned by a user so transfers can update
    /// `LatestPeriod` without scanning contract storage.
    WrapPeriods(Address),
    /// Stores the admin-controlled transfer fee configuration.
    TransferFee,
    /// Temporary reentrancy guard for transfer calls.
    TransferGuard,
    /// Stores the highest storage migration version already applied.
    MigrationVersion,
}
