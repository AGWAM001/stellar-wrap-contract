#[cfg(test)]
extern crate std;
use soroban_sdk::{contracttype, Address, BytesN, Symbol};

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WrapState { Draft = 1, Pending = 2, Active = 3, Archived = 4, Cancelled = 5 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrapLifecycleFSM { pub state: WrapState, pub updated_at: u64 }

impl WrapLifecycleFSM {
    pub fn new(initial_state: WrapState, now: u64) -> Self { Self { state: initial_state, updated_at: now } }
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
        if self.can_transition_to(&next) { self.state = next; self.updated_at = now; true } else { false }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrapRecord {
    pub timestamp: u64, pub data_hash: BytesN<32>, pub archetype: Symbol,
    pub period: u64, pub fsm: WrapLifecycleFSM,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractHealth {
    pub initialized: bool, pub has_admin: bool, pub has_signing_key: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeParams {
    pub base_fee: i128, pub per_kib_fee: i128, pub scale_step_kib: u64, pub max_fee: i128,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin, AdminPubKey, PendingAdmin, Wrap(Address, u64),
    WrapCount(Address), LatestPeriod(Address), MigrationVersion,
    UserPeriods(Address), TotalWrapCount, TotalRevoked, AliasHash(Address),
    Name, Symbol, Paused, StorageBytes, FeeParams,
}