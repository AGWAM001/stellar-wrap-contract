use soroban_sdk::{panic_with_error, symbol_short, Address, BytesN, Env};

use crate::{ContractError, DataKey};

/// Reads the stored admin or panics with `NotInitialized`.
pub(crate) fn read_admin(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized))
}

pub(crate) fn initialize(e: Env, admin: Address, admin_pubkey: BytesN<32>) {
    if e.storage().instance().has(&DataKey::Admin) {
        panic_with_error!(e, ContractError::AlreadyInitialized);
    }
    e.storage().instance().set(&DataKey::Admin, &admin);
    e.storage()
        .instance()
        .set(&DataKey::AdminPubKey, &admin_pubkey);
    e.events().publish((symbol_short!("init"),), admin);
}

pub(crate) fn update_admin(e: Env, new_admin: Address) {
    let current_admin = read_admin(&e);
    current_admin.require_auth();
    e.storage().instance().set(&DataKey::Admin, &new_admin);
    e.storage().instance().remove(&DataKey::PendingAdmin);

    e.events().publish(
        (symbol_short!("admin"), symbol_short!("updated")),
        (current_admin, new_admin),
    );
}



pub(crate) fn set_pause(e: Env, paused: bool) {
    read_admin(&e).require_auth();
    e.storage().instance().set(&DataKey::Paused, &paused);
    e.events().publish((symbol_short!("pause"),), paused);
}

pub(crate) fn is_paused(e: &Env) -> bool {
    e.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

pub(crate) fn require_not_paused(e: &Env) {
    if is_paused(e) {
        panic_with_error!(e, ContractError::Paused);
    }
}

/// Marks a storage migration as applied. A version can only be applied once and
/// versions must move forward, so an upgrade shipping a migration cannot corrupt
/// storage by replaying it.
pub(crate) fn migrate(e: Env, version: u32) {
    read_admin(&e).require_auth();

    let applied = migration_version(&e);
    if version <= applied {
        panic_with_error!(e, ContractError::MigrationAlreadyApplied);
    }

    e.storage()
        .instance()
        .set(&DataKey::MigrationVersion, &version);
}

pub(crate) fn migration_version(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::MigrationVersion)
        .unwrap_or(0)
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

pub(crate) fn propose_admin(e: Env, new_admin: Address) {
    let current_admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

    current_admin.require_auth();

    if e.storage().instance().has(&DataKey::PendingAdmin) {
        panic_with_error!(e, ContractError::AdminTransferProposalExists);
    }

    e.storage().instance().set(&DataKey::PendingAdmin, &new_admin);
}

pub(crate) fn accept_admin(e: Env) {
    let _: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

    let pending_admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::PendingAdmin)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NoAdminTransferProposal));

    pending_admin.require_auth();

    e.storage().instance().set(&DataKey::Admin, &pending_admin);
    e.storage().instance().remove(&DataKey::PendingAdmin);
}

pub(crate) fn cancel_proposed_admin(e: Env) {
    let current_admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

    current_admin.require_auth();

    if !e.storage().instance().has(&DataKey::PendingAdmin) {
        panic_with_error!(e, ContractError::NoAdminTransferProposal);
    }

    e.storage().instance().remove(&DataKey::PendingAdmin);
}

pub(crate) fn get_pending_admin(e: Env) -> Option<Address> {
    e.storage().instance().get(&DataKey::PendingAdmin)
}

pub(crate) fn set_name(e: Env, name: soroban_sdk::String) {
    let current_admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

    current_admin.require_auth();
    e.storage().instance().set(&DataKey::Name, &name);
}

pub(crate) fn set_symbol(e: Env, symbol: soroban_sdk::String) {
    let current_admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

    current_admin.require_auth();
    e.storage().instance().set(&DataKey::Symbol, &symbol);
}


