#![no_std]

#[cfg(test)]
extern crate std;

use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, String, Symbol, Vec};

mod admin;
mod errors;
mod mint;
mod queries;
mod storage_types;
mod transfer;

pub use errors::ContractError;
pub use storage_types::{ContractHealth, DataKey, TransferFeeConfig, WrapRecord};

#[contract]
pub struct StellarWrapContract;

#[contractimpl]
impl StellarWrapContract {
    pub fn initialize(e: Env, admin: Address, admin_pubkey: BytesN<32>) {
        admin::initialize(e, admin, admin_pubkey);
    }

    pub fn update_admin(e: Env, new_admin: Address) {
        admin::update_admin(e, new_admin);
    }

    /// Configures the token-denominated fee charged by `transfer_wrap`.
    ///
    /// Only the current admin may update the configuration. An amount of zero
    /// enables fee-free transfers without removing the configured token and
    /// recipient.
    pub fn set_transfer_fee(e: Env, token: Address, recipient: Address, amount: i128) {
        admin::set_transfer_fee(e, token, recipient, amount);
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
    }

    /// Transfers one wrap record and atomically charges the configured fee.
    ///
    /// The current owner (`from`) must authorize the invocation. The record is
    /// moved only if fee payment succeeds; any token-contract failure rolls the
    /// entire invocation back.
    pub fn transfer_wrap(e: Env, from: Address, to: Address, period: u64) {
        transfer::transfer_wrap(e, from, to, period);
    }

    /// Backfills the ownership-period index for records minted before transfer
    /// support was deployed. Admin-only and callable once per user.
    pub fn backfill_wrap_periods(e: Env, user: Address, periods: Vec<u64>) {
        transfer::backfill_wrap_periods(e, user, periods);
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

    pub fn get_admin(e: Env) -> Option<Address> {
        queries::get_admin(e)
    }

    pub fn get_transfer_fee(e: Env) -> Option<TransferFeeConfig> {
        queries::get_transfer_fee(e)
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
#[cfg(test)]
mod transfer_test;
