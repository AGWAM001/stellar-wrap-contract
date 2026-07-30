use soroban_sdk::{panic_with_error, symbol_short, Address, Env};

use crate::{ContractError, DataKey, WrapRecord};

pub(crate) fn revoke_wrap(e: Env, user: Address, period: u64) {
    user.require_auth();

    let wrap_key = DataKey::Wrap(user.clone(), period);
    let record: WrapRecord = e
        .storage()
        .persistent()
        .get(&wrap_key)
        .unwrap_or_else(|| panic_with_error!(&e, ContractError::InvalidSignature));

    e.storage().persistent().remove(&wrap_key);

    let count_key = DataKey::WrapCount(user.clone());
    let current_count: u32 = e.storage().persistent().get(&count_key).unwrap_or(0);
    if current_count > 0 {
        let next_count = current_count - 1;
        e.storage().persistent().set(&count_key, &next_count);
    }

    e.events()
        .publish((symbol_short!("revoke"), user, period), record.archetype);
}
