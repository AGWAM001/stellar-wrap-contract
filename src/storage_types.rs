#[cfg(any(test, feature = "testutils"))]
extern crate std;
use soroban_sdk::{contracttype, Address, BytesN, String, Symbol};

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
        matches!(
            (&self.state, next),
            (WrapState::Draft, WrapState::Pending)
                | (WrapState::Draft, WrapState::Cancelled)
                | (WrapState::Pending, WrapState::Active)
                | (WrapState::Pending, WrapState::Cancelled)
                | (WrapState::Active, WrapState::Archived)
                | (WrapState::Active, WrapState::Cancelled)
        )
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
    pub description: Option<String>,
    pub image_url: Option<String>,
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
    /// Stores a proposed new admin address during two-step transfer.
    PendingAdmin,
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
    /// Stores a list of periods a user has minted wraps for.
    UserPeriods(Address),
    /// Stores the total number of successful wrap mints across all users.
    TotalWrapCount,
    /// Stores the total number of wrap records revoked on-chain.
    TotalRevoked,
    /// Stores a user-controlled 32-byte alias hash for privacy-preserving profile display.
    AliasHash(Address),
    /// Stores the token display name, if overridden by an admin.
    /// Falls back to a hardcoded default when unset — see `queries::name`.
    Name,
    /// Stores the token symbol, if overridden by an admin.
    /// Falls back to a hardcoded default when unset — see `queries::symbol`.
    Symbol,
    /// Emergency pause state flag.
    Paused,

    // New instance storage keys for accounting / fee system:
    /// Estimated persistent storage bytes used by this contract (instance-level)
    StorageBytes,
    /// Params for the algorithmic fee function (instance-level)
    FeeParams,
}
