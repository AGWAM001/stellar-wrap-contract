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

use soroban_sdk::{contract, contractimpl, panic_with_error, Address, Bytes, BytesN, Env, String, Symbol};

mod admin;
mod alias;
mod errors;
mod mint;
mod queries;
mod revoke;
mod signature;
mod storage_types;

pub use errors::ContractError;
pub use storage_types::{ContractHealth, DataKey, WrapLifecycleFSM, WrapRecord, WrapState};

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

    pub fn pause(e: Env) {
        admin::set_pause(e, true);
    }

    pub fn unpause(e: Env) {
        admin::set_pause(e, false);
    }

    pub fn is_paused(e: Env) -> bool {
        admin::is_paused(&e)
    }

    /// Records that the storage migration `version` has been applied.
    /// Admin-only, and each version can only be applied once.
    pub fn migrate(e: Env, version: u32) {
        admin::migrate(e, version);
    }

    pub fn migration_version(e: Env) -> u32 {
        admin::migration_version(&e)
    }

    pub fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        admin::upgrade(e, new_wasm_hash);
    }

    pub fn propose_admin(e: Env, new_admin: Address) {
        admin::propose_admin(e, new_admin);
    }

    pub fn accept_admin(e: Env) {
        admin::accept_admin(e);
    }

    pub fn cancel_proposed_admin(e: Env) {
        admin::cancel_proposed_admin(e);
    }

    pub fn get_pending_admin(e: Env) -> Option<Address> {
        admin::get_pending_admin(e)
    }

    pub fn set_name(e: Env, name: String) {
        admin::set_name(e, name);
    }

    pub fn set_symbol(e: Env, symbol: String) {
        admin::set_symbol(e, symbol);
    }

    pub fn mint_wrap(
        e: Env,
        user: Address,
        period: u64,
        archetype: Symbol,
        data_hash: BytesN<32>,
        payload_version: u32,
        signature: BytesN<64>,
    ) {
        mint::mint_wrap(e, user, period, archetype, data_hash, payload_version, signature);
    }

    pub fn transition_wrap_state(
        e: Env,
        user: Address,
        period: u64,
        next_state: WrapState,
    ) {
        mint::transition_wrap_state(e, user, period, next_state);
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

    pub fn total_wrap_count(e: Env) -> u32 {
        queries::total_wrap_count(e)
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

    /// Extend the TTL (time-to-live) for all persistent storage entries belonging to a user.
    ///
    /// Soroban persistent storage entries expire after their TTL lapses. This function lets
    /// anyone renew a user's wrap records so they remain accessible indefinitely.
    ///
    /// # TTL Lifecycle
    ///
    /// All persistent storage entries (wraps, balance, latest-period marker) are stored with
    /// a TTL of ~1 year (17280 × 365 ledgers) at creation time.
    ///
    /// **Automatic renewal (metadata only):** When `mint_wrap` is called, the `WrapCount`
    /// and `LatestPeriod` metadata keys are automatically extended by another ~1 year.
    /// This keeps the user's balance-of and latest-wrap lookup alive for active users
    /// without any manual intervention.
    ///
    /// **Manual renewal (individual wraps):** Historical wrap records for specific
    /// `(user, period)` pairs are **not** automatically extended on new mints.
    /// Anyone can call this `extend_ttl` function to renew a specific wrap record.
    ///
    /// **Bulk renewal (admin):** The [`renew_all_ttls`] function allows the admin to
    /// extend the TTL of all metadata keys for a user. Full wrap-enumeration renewal
    /// requires period tracking (see Issue #90).
    ///
    /// **Expiry risk:** Without periodic renewal, the first wraps of an active multi-year
    /// user could expire after ~1 year, even though the user is still participating.
    /// Off-chain bots or the admin should call `extend_ttl` for historical periods
    /// of active users to prevent data loss.
    ///
    /// # Parameters
    /// - `user`: The address whose storage entries will be extended.
    /// - `period`: The specific wrap period whose record TTL will be extended.
    pub fn extend_ttl(e: Env, user: Address, period: u64) {
        let wrap_key = DataKey::Wrap(user.clone(), period);
        let ttl = 17280 * 365; // ~1 year in ledgers

        if e.storage().persistent().has(&wrap_key) {
            e.storage().persistent().extend_ttl(&wrap_key, ttl, ttl);
        }

        let count_key = DataKey::WrapCount(user.clone());
        if e.storage().persistent().has(&count_key) {
            e.storage().persistent().extend_ttl(&count_key, ttl, ttl);
        }

        let latest_key = DataKey::LatestPeriod(user);
        if e.storage().persistent().has(&latest_key) {
            e.storage().persistent().extend_ttl(&latest_key, ttl, ttl);
        }

        e.storage().instance().extend_ttl(ttl, ttl);
    }

    /// Admin-only function to extend TTL for all metadata keys associated with a user.
    ///
    /// This extends the TTL (time-to-live) for `WrapCount` and `LatestPeriod` storage
    /// entries, keeping the user's balance and latest-period data alive. It also extends
    /// the contract instance TTL. Individual historical wrap records are **not** extended
    /// — full per-wrap renewal requires period enumeration, tracked as Issue #90.
    ///
    /// # Motivation
    ///
    /// Active users who mint new wraps periodically will have their metadata keys
    /// automatically renewed by [`mint_wrap`]. However, if there is a long gap between
    /// mints, the metadata keys could expire. This function lets the admin proactively
    /// renew a user's metadata without requiring a new mint.
    ///
    /// # Parameters
    /// - `user`: The address whose metadata storage TTLs will be extended.
    ///
    /// # Authorization
    /// Requires authorization from the **admin**.
    ///
    /// # Panics
    /// - [`ContractError::NotInitialized`] if the contract has not been initialized.
    pub fn renew_all_ttls(e: Env, user: Address) {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));
        admin.require_auth();

        let ttl = 17280 * 365; // ~1 year in ledgers

        let count_key = DataKey::WrapCount(user.clone());
        if e.storage().persistent().has(&count_key) {
            e.storage().persistent().extend_ttl(&count_key, ttl, ttl);
        }

        let latest_key = DataKey::LatestPeriod(user);
        if e.storage().persistent().has(&latest_key) {
            e.storage().persistent().extend_ttl(&latest_key, ttl, ttl);
        }

        e.storage().instance().extend_ttl(ttl, ttl);
    }

    /// Return the current admin address, or `None` if the contract is not yet initialized.
    pub fn get_admin(e: Env) -> Option<Address> {
        queries::get_admin(e)
    }

    pub fn health(e: Env) -> ContractHealth {
        queries::health(e)
    }

    /// Set or update the caller's alias hash.
    ///
    /// Only the `user` themselves can call this — `require_auth` is enforced
    /// inside the alias module. The hash is stored as opaque 32-byte data so
    /// no raw personal information ever touches the chain.
    pub fn set_alias_hash(e: Env, user: Address, alias_hash: BytesN<32>) {
        alias::set_alias_hash(e, user, alias_hash);
    }

    /// Return the alias hash for `user`, or `None` if one has not been set.
    pub fn get_alias_hash(e: Env, user: Address) -> Option<BytesN<32>> {
        alias::get_alias_hash(e, user)
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

    pub fn revoke_wrap(e: Env, user: Address, period: u64, reason_hash: BytesN<32>) {
        revoke::revoke_wrap(e, user, period, reason_hash);
    }

    pub fn total_revoked(e: Env) -> u64 {
        queries::total_revoked(e)
    }
}

#[cfg(test)]
mod security_test;
#[cfg(test)]
mod test;
#[cfg(test)]
mod test_utils;
