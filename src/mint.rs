use soroban_sdk::{
    panic_with_error, symbol_short, xdr::ToXdr, Address, Bytes, BytesN, Env, Symbol,
};

use crate::{ContractError, DataKey, WrapRecord};
use crate::storage_accounting;

const TTL_ONE_YEAR: u32 = 17_280 * 365;

fn validate_period(e: &Env, period: u64) {
    let year = period / 100;
    let month = period % 100;

    if year < 2024 || year > 2100 || month < 1 || month > 12 {
        panic_with_error!(e, ContractError::InvalidPeriod);
    }
}

fn get_admin_pubkey(e: &Env) -> BytesN<32> {
    e.storage()
        .instance()
        .get(&DataKey::AdminPubKey)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized))
}

fn build_payload(
    e: &Env,
    contract: &Address,
    user: &Address,
    period: u64,
    archetype: &Symbol,
    data_hash: &BytesN<32>,
) -> Bytes {
    let mut payload = Bytes::new(e);
    payload.append(&contract.to_xdr(e));
    payload.append(&user.clone().to_xdr(e));
    payload.append(&period.to_xdr(e));
    payload.append(&archetype.clone().to_xdr(e));
    payload.append(&data_hash.clone().to_xdr(e));
    payload
}

pub(crate) fn mint_wrap(
    e: Env,
    user: Address,
    period: u64,
    archetype: Symbol,
    data_hash: BytesN<32>,
    signature: BytesN<64>,
) {
    user.require_auth();
    validate_period(&e, period);

    let admin_pubkey = get_admin_pubkey(&e);
    let payload = build_payload(
        &e,
        &e.current_contract_address(),
        &user,
        period,
        &archetype,
        &data_hash,
    );

    e.crypto()
        .ed25519_verify(&admin_pubkey, &payload, &signature);

    let wrap_key = DataKey::Wrap(user.clone(), period);
    if e.storage().persistent().has(&wrap_key) {
        panic_with_error!(e, ContractError::WrapAlreadyExists);
    }

    let record = WrapRecord {
        timestamp: e.ledger().timestamp(),
        data_hash,
        archetype: archetype.clone(),
        period,
    };

    e.storage().persistent().set(&wrap_key, &record);
    e.storage()
        .persistent()
        .extend_ttl(&wrap_key, TTL_ONE_YEAR, TTL_ONE_YEAR);

    // Account for estimated storage bytes for new wrap record
    storage_accounting::add_storage_bytes(
        &e,
        storage_accounting::estimate_wrap_bytes_new(),
    );

    // Update wrap count and account for count entry if first insert
    let count_key = DataKey::WrapCount(user.clone());
    let current_count: u32 = e.storage().persistent().get(&count_key).unwrap_or(0);
    let next_count = current_count + 1;
    e.storage().persistent().set(&count_key, &next_count);
    e.storage()
        .persistent()
        .extend_ttl(&count_key, TTL_ONE_YEAR, TTL_ONE_YEAR);

    if current_count == 0 {
        storage_accounting::add_storage_bytes(
            &e,
            storage_accounting::estimate_wrapcount_bytes_new(),
        );
    }

    // LatestPeriod: if newly inserted, account for bytes
    let latest_key = DataKey::LatestPeriod(user.clone());
    let current_latest: u64 = e.storage().persistent().get(&latest_key).unwrap_or(0);
    if period > current_latest {
        // If latest did not exist before (==0) we'll consider it a new entry when current_latest == 0
        let was_missing = current_latest == 0;
        e.storage().persistent().set(&latest_key, &period);
        e.storage()
            .persistent()
            .extend_ttl(&latest_key, TTL_ONE_YEAR, TTL_ONE_YEAR);
        if was_missing {
            storage_accounting::add_storage_bytes(
                &e,
                storage_accounting::estimate_latest_bytes_new(),
            );
        }
    }

    // UserPeriods: if we push a new period value, account for it
    let user_periods_key = DataKey::UserPeriods(user.clone());
    let mut periods: soroban_sdk::Vec<u64> = e
        .storage()
        .persistent()
        .get(&user_periods_key)
        .unwrap_or(soroban_sdk::Vec::new(&e));
    
    if !periods.contains(period) {
        periods.push_back(period);
        e.storage().persistent().set(&user_periods_key, &periods);
        e.storage()
            .persistent()
            .extend_ttl(&user_periods_key, TTL_ONE_YEAR, TTL_ONE_YEAR);

        // For simplicity account for the user periods entry cost (conservative)
        storage_accounting::add_storage_bytes(
            &e,
            storage_accounting::estimate_userperiods_bytes_new(),
        );
    }

    e.events()
        .publish((symbol_short!("mint"), user, period), archetype);
}
