#![no_std]

use soroban_sdk::{
    contracterror,
    panic_with_error,
    symbol_short,
    Address, BytesN, Env, Symbol,
};

/// Errors returned by the StellarWrap contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    /// A wrap record for this `(user, period)` pair already exists. (code 4)
    WrapAlreadyExists = 4,
}

#[contractimpl]
impl StellarWrapContract {
    pub fn initialize(e: Env, admin: Address, admin_pubkey: BytesN<32>) {
        e.storage().instance().set(&symbol_short!("admin"), &admin);
        e.storage().instance().set(&symbol_short!("admin_pubkey"), &admin_pubkey);
    }

    pub fn mint_wrap(
        e: Env,
        user: Address,
        period: u64,
        archetype: Symbol,
        data_hash: BytesN<32>,
        _signature: BytesN<64>,
    ) {
        let key = (user.clone(), period);
        if e.storage().persistent().has(&key) {
            panic_with_error!(e, ContractError::WrapAlreadyExists);
        }
        let record = WrapRecord {
            timestamp: e.ledger().timestamp(),
            data_hash,
            archetype,
            period,
        };
        e.storage().persistent().set(&key, &record);
    }

    pub fn get_wrap(e: Env, user: Address, period: u64) -> Option<WrapRecord> {
        let key = (user, period);
        e.storage().persistent().get(&key)
    }

    pub fn balance_of(e: Env, user: Address) -> u32 {
        let key = (user.clone(), 1u64);
        if e.storage().persistent().has(&key) {
            1
        } else {
            0
        }
    }
}

// Re-export for tests
pub use StellarWrapContractClient;