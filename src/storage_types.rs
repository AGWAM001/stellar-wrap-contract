#[cfg(test)]
extern crate std;
use soroban_sdk::{contracttype, Address, BytesN, Symbol};

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WrapState {
    Draft = 1,
    Pending = 2,
    Active = 3,
    Archived = 4,
    Cancelled = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrapLifecycleFSM {
    pub state: WrapState,
    pub updated_at: u64,
}

impl WrapLifecycleFSM {
    pub fn new(initial_state: WrapState, now: u64) -> Self {
        Self {
            state: initial_state,
            updated_at: now,
        }
    }

    pub fn can_transition_to(&self, next: &WrapState) -> bool {
        match (&self.state, next) {
            (WrapState::Draft, WrapState::Pending) => true,
            (WrapState::Draft, WrapState::Cancelled) => true,
            (WrapState::Pending, WrapState::Active) => true,
            (WrapState::Pending, WrapState::Cancelled) => true,
            (WrapState::Active, WrapState::Archived) => true,
            (WrapState::Active, WrapState::Cancelled) => true,
            _ => false,
        }
    }

    pub fn transition_to(&mut self, next: WrapState, now: u64) -> bool {
        if self.can_transition_to(&next) {
            self.state = next;
            self.updated_at = now;
            true
        } else {
            false
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrapRecord {
    pub timestamp: u64,
    pub data_hash: BytesN<32>,
    pub archetype: Symbol,
    pub period: u64, // Standardized to u64 for better indexing/sorting
    pub fsm: WrapLifecycleFSM,
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
    /// Stores the paused state of the contract.
    Paused,
    /// Stores the total number of successful wrap mints across all users.
    TotalWrapCount,
    /// Stores the total number of wrap records revoked on-chain.
    TotalRevoked,
    /// Stores a user-controlled 32-byte alias hash for privacy-preserving profile display.
    AliasHash(Address),
}
