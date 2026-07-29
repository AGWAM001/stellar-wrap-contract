//! # Stellar Wrap Registry
//!
//! A Soroban smart contract on Stellar that records timestamped data-wrap
//! commitments on-chain. Each wrap binds a user address, a period
//! (`YYYYMM`), an archetype label, and a SHA-256 data hash into an
//! immutable record.
//!
//! ## Security
//!
//! Minting requires an Ed25519 signature from the configured admin key
//! over the full payload (contract ID, user, period, archetype, data hash).
//! This prevents unauthorized wraps even if the caller contract is
//! compromised. The admin address controls the public-key rotation.

#![no_std]

#[cfg(test)]
extern crate std;

use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, String, Symbol};

mod admin;
mod errors;
mod mint;
mod queries;
mod revoke;
mod storage_types;

pub use errors::ContractError;
pub use storage_types::{ContractHealth, DataKey, WrapRecord};

#[contract]
pub struct StellarWrapContract;
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
        admin::initialize(e, admin, admin_pubkey);
    }

    pub fn update_admin(e: Env, new_admin: Address) {
        admin::update_admin(e, new_admin);
    }

    pub fn unpause(e: Env) {
        admin::unpause(e);
    }

    /// Records that the storage migration `version` has been applied.
    /// Admin-only, and each version can only be applied once.
    pub fn migrate(e: Env, version: u32) {
        admin::migrate(e, version);
    }

    pub fn migration_version(e: Env) -> u32 {
        admin::migration_version(&e)
    }

    pub fn mint_wrap(
        e: Env,
        user: Address,
        period: u64,
        archetype: Symbol,
        data_hash: BytesN<32>,
        signature: BytesN<64>,
    ) {
        mint::mint_wrap(e, user, period, archetype, data_hash, signature);
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

    pub fn revoke_wrap(e: Env, user: Address, period: u64) {
        revoke::revoke_wrap(e, user, period);
    }

    pub fn get_wrap(e: Env, user: Address, period: u64) -> Option<WrapRecord> {
        queries::get_wrap(e, user, period)
    }

    /// Returns the mint timestamp for a known user-period.
    /// The timestamp reflects ledger time, not wall-clock time.
    /// Returns `None` if no mint has occurred for the given user-period.
    pub fn get_mint_timestamp(e: Env, user: Address, period: u64) -> Option<u64> {
        queries::get_mint_timestamp(e, user, period)
    }

    pub fn balance_of(e: Env, user: Address) -> i128 {
        queries::balance_of(e, user)
    }

    pub fn verify_data(e: Env, user: Address, period: u64, data: Bytes) -> bool {
        queries::verify_data(e, user, period, data)
    }

    pub fn get_latest_wrap(e: Env, user: Address) -> Option<WrapRecord> {
        queries::get_latest_wrap(e, user)
    }

    pub fn get_wraps(e: Env, user: Address, start: u32, limit: u32) -> soroban_sdk::Vec<WrapRecord> {
        queries::get_wraps(e, user, start, limit)
    }

    pub fn get_admin(e: Env) -> Option<Address> {
        queries::get_admin(e)
    }

    pub fn health(e: Env) -> ContractHealth {
        queries::health(e)
    }

    pub fn name(e: Env) -> String {
        queries::name(e)
    }

    pub fn symbol(e: Env) -> String {
        queries::symbol(e)
    }

    pub fn decimals(e: Env) -> u32 {
        queries::decimals(e)
    }
}

#[cfg(test)]
mod security_test;
#[cfg(test)]
mod test;
#[cfg(test)]
mod test_utils;
