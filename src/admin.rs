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

pub(crate) fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
    let current_admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

    current_admin.require_auth();

    // Emit audit event with the requested WASM hash before performing the upgrade
    e.events()
        .publish((symbol_short!("upgrade"),), new_wasm_hash.clone());

    // Update the contract WASM with the provided hash
    e.deployer().update_current_contract_wasm(new_wasm_hash);
}

pub(crate) fn revoke_wrap(e: Env, user: Address, period: u64) {
    let current_admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

    current_admin.require_auth();

    let wrap_key = DataKey::Wrap(user.clone(), period);
    if !e.storage().persistent().has(&wrap_key) {
        panic_with_error!(e, ContractError::WrapNotFound);
    }

    e.storage().persistent().remove(&wrap_key);

    let count_key = DataKey::WrapCount(user.clone());
    let current_count: u32 = e.storage().persistent().get(&count_key).unwrap_or(0);
    let next_count = current_count.saturating_sub(1);
    e.storage().persistent().set(&count_key, &next_count);

    let total_revoked_key = DataKey::TotalRevoked;
    let current_total: u64 = e.storage().instance().get(&total_revoked_key).unwrap_or(0);
    let next_total = current_total + 1;
    e.storage().instance().set(&total_revoked_key, &next_total);

    e.events()
        .publish((symbol_short!("revoke"), user, period), ());
}
