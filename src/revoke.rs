use soroban_sdk::{panic_with_error, symbol_short, Address, Env};

use crate::{ContractError, DataKey, WrapRecord};
use crate::storage_accounting;

pub(crate) fn revoke_wrap(e: Env, user: Address, period: u64) {
    user.require_auth();

    let wrap_key = DataKey::Wrap(user.clone(), period);
    let record: WrapRecord = e.storage().persistent().get(&wrap_key)
        .unwrap_or_else(|| panic_with_error!(&e, ContractError::InvalidSignature));

    // Remove the wrap entry and subtract estimated bytes
    e.storage().persistent().remove(&wrap_key);
    storage_accounting::sub_storage_bytes(&e, storage_accounting::estimate_wrap_bytes_new());

    let count_key = DataKey::WrapCount(user.clone());
    let current_count: u32 = e.storage().persistent().get(&count_key).unwrap_or(0);
    if current_count > 0 {
        let next_count = current_count - 1;
        e.storage().persistent().set(&count_key, &next_count);
        // If count became zero, we consider removing the count entry overhead
        if next_count == 0 {
            storage_accounting::sub_storage_bytes(&e, storage_accounting::estimate_wrapcount_bytes_new());
            // Optionally remove the key entirely (keep it set to 0 for now to match existing behavior)
        }
    }

    // Note: We do not attempt to compact user_periods vector here — that would require scanning and
    // potentially expensive writes. A future migration tool can recompute the storage bytes precisely.

    e.events()
        .publish((symbol_short!("revoke"), user, period), record.archetype);
}
