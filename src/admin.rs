use soroban_sdk::{panic_with_error, symbol_short, Address, BytesN, Env};

use crate::{ContractError, DataKey};

pub(crate) fn initialize(e: Env, admin: Address, admin_pubkey: BytesN<32>) {
    if e.storage().instance().has(&DataKey::Admin) {
        panic_with_error!(e, ContractError::AlreadyInitialized);
    }
    e.storage().instance().set(&DataKey::Admin, &admin);
    e.storage()
        .instance()
        .set(&DataKey::AdminPubKey, &admin_pubkey);
}

pub(crate) fn update_admin(e: Env, new_admin: Address) {
    let current_admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

    current_admin.require_auth();
    e.storage().instance().set(&DataKey::Admin, &new_admin);
}

const TTL_ONE_YEAR: u32 = 17_280 * 365;

pub(crate) fn revoke_wrap(e: Env, user: Address, period: u64) {
    let admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(&e, ContractError::NotInitialized));
    admin.require_auth();

    let wrap_key = DataKey::Wrap(user.clone(), period);
    if !e.storage().persistent().has(&wrap_key) {
        panic_with_error!(&e, ContractError::WrapNotFound);
    }
    e.storage().persistent().remove(&wrap_key);

    let count_key = DataKey::WrapCount(user.clone());
    let current_count: u32 = e.storage().persistent().get(&count_key).unwrap_or(0);
    if current_count > 0 {
        let new_count = current_count - 1;
        e.storage().persistent().set(&count_key, &new_count);
        e.storage()
            .persistent()
            .extend_ttl(&count_key, TTL_ONE_YEAR, TTL_ONE_YEAR);
    }

    let latest_key = DataKey::LatestPeriod(user.clone());
    let current_latest: u64 = e.storage().persistent().get(&latest_key).unwrap_or(0);
    if period == current_latest {
        e.storage().persistent().remove(&latest_key);
    }

    e.events()
        .publish((symbol_short!("revoke"), user, period), ());
}
