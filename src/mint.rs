use soroban_sdk::{panic_with_error, Address, BytesN, Env, Symbol};

use crate::events::{MintEventData, MintEventType};
use crate::storage_accounting;
use crate::storage_types::{WrapLifecycleFSM, WrapState};
use crate::{signature::verify_mint_signature, ContractError, DataKey, WrapRecord};

const TTL_ONE_YEAR: u32 = 17_280 * 365;
/// TTL for temporary storage entries (~1 day in ledgers at 5s/ledger).
/// Used for non-critical data migrated from Instance to Temporary storage.
pub(crate) const TTL_TEMP: u32 = 17_280;
pub const CURRENT_PAYLOAD_VERSION: u32 = 1;
/// Default expiration duration for unverified wraps: 7 days in seconds.
const DEFAULT_EXPIRATION_SECONDS: u64 = 7 * 24 * 60 * 60;

fn validate_period(e: &Env, period: u64) {
    let year = period / 100;
    let month = period % 100;

    if !(2024..=2100).contains(&year) || !(1..=12).contains(&month) {
        panic_with_error!(e, ContractError::InvalidPeriod);
    }
}

fn validate_payload_version(e: &Env, version: u32) {
    if version != CURRENT_PAYLOAD_VERSION {
        panic_with_error!(e, ContractError::InvalidSignature);
    }
}

fn get_admin_pubkey(e: &Env) -> BytesN<32> {
    e.storage()
        .instance()
        .get(&DataKey::AdminPubKey)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized))
}

pub(crate) fn mint_wrap(
    e: Env,
    user: Address,
    period: u64,
    archetype: Symbol,
    data_hash: BytesN<32>,
    payload_version: u32,
    signature: BytesN<64>,
) {
    crate::admin::require_not_paused(&e);
    user.require_auth();
    validate_period(&e, period);
    validate_payload_version(&e, payload_version);

    let admin_pubkey = get_admin_pubkey(&e);
    let _ = verify_mint_signature(
        &e,
        &admin_pubkey,
        &e.current_contract_address(),
        &user,
        period,
        &archetype,
        &data_hash,
        payload_version,
        &signature,
    );

    let wrap_key = DataKey::Wrap(user.clone(), period);
    if e.storage().persistent().has(&wrap_key) {
        panic_with_error!(e, ContractError::WrapAlreadyExists);
    }

    let now = e.ledger().timestamp();
    let record = WrapRecord {
        timestamp: now,
        data_hash,
        archetype: archetype.clone(),
        period,
        fsm: WrapLifecycleFSM::new(WrapState::Active, now),
        description: None,
        image_url: None,
    };

    e.storage().persistent().set(&wrap_key, &record);
    e.storage()
        .persistent()
        .extend_ttl(&wrap_key, TTL_ONE_YEAR, TTL_ONE_YEAR);

    // Account for estimated storage bytes for new wrap record
    storage_accounting::add_storage_bytes(&e, storage_accounting::estimate_wrap_bytes_new());

    // Update wrap count and account for count entry if first insert
    let count_key = DataKey::WrapCount(user.clone());
    let current_count: u32 = e.storage().persistent().get(&count_key).unwrap_or(0);
    let next_count = current_count + 1;
    e.storage().persistent().set(&count_key, &next_count);
    e.storage()
        .persistent()
        .extend_ttl(&count_key, TTL_ONE_YEAR, TTL_ONE_YEAR);

    let total_key = DataKey::TotalWrapCount;
    let current_total: u32 = e.storage().persistent().get(&total_key).unwrap_or(0);
    let next_total = current_total + 1;
    e.storage().persistent().set(&total_key, &next_total);
    e.storage()
        .persistent()
        .extend_ttl(&total_key, TTL_ONE_YEAR, TTL_ONE_YEAR);

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

    e.events().publish(
        (MintEventType::Mint.to_symbol(&e), user.clone(), period),
        MintEventData::Mint(user, period, archetype),
    );
}

pub(crate) fn transition_wrap_state(e: Env, user: Address, period: u64, next_state: WrapState) {
    crate::admin::require_not_paused(&e);
    user.require_auth();

    let wrap_key = DataKey::Wrap(user.clone(), period);
    let mut record: WrapRecord = e
        .storage()
        .persistent()
        .get(&wrap_key)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::WrapNotFound));

    let now = e.ledger().timestamp();
    if !record.fsm.transition_to(next_state, now) {
        panic_with_error!(e, ContractError::InvalidStateTransition);
    }

    e.storage().persistent().set(&wrap_key, &record);
    e.storage()
        .persistent()
        .extend_ttl(&wrap_key, TTL_ONE_YEAR, TTL_ONE_YEAR);

    e.events().publish(
        (MintEventType::Transition.to_symbol(&e), user.clone(), period),
        MintEventData::Transition(user, period, next_state),
    );
}

// ─── Expiration mechanism ────────────────────────────────────────────────

/// Returns the configured expiration duration for unverified wraps.
/// Defaults to 7 days (604,800 seconds) if not set by admin.
pub(crate) fn get_expiration_duration(e: &Env) -> u64 {
    e.storage()
        .instance()
        .get(&DataKey::ExpirationDuration)
        .unwrap_or(DEFAULT_EXPIRATION_SECONDS)
}

/// Admin-only: sets the expiration duration (in seconds) for unverified wraps.
/// Wraps in Draft or Pending state that remain unverified beyond this duration
/// can be expired by anyone via [`expire_wrap`].
pub(crate) fn set_expiration_duration(e: &Env, duration: u64) {
    crate::admin::read_admin(e).require_auth();
    if duration == 0 {
        panic_with_error!(e, ContractError::InvalidExpirationDuration);
    }
    e.storage()
        .instance()
        .set(&DataKey::ExpirationDuration, &duration);
}

/// Expires an unverified wrap if its expiration deadline has passed.
///
/// A wrap can be expired if:
/// - It is in `Draft` or `Pending` state (unverified).
/// - The ledger timestamp exceeds `fsm.updated_at + expiration_duration`.
///
/// Callable by anyone — the function enforces objective time-based criteria.
/// Wraps already in `Active`, `Archived`, `Cancelled`, or `Expired` state
/// will cause the FSM transition to fail with [`ContractError::InvalidStateTransition`].
///
/// Expired wraps remain in persistent storage; no storage bytes are reclaimed.
pub(crate) fn expire_wrap(e: Env, user: Address, period: u64) {
    crate::admin::require_not_paused(&e);

    let wrap_key = DataKey::Wrap(user.clone(), period);
    let mut record: WrapRecord = e
        .storage()
        .persistent()
        .get(&wrap_key)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::WrapNotFound));

    let now = e.ledger().timestamp();
    let duration = get_expiration_duration(&e);
    let expires_at = record.fsm.updated_at.saturating_add(duration);

    if now < expires_at {
        panic_with_error!(e, ContractError::WrapNotExpired);
    }

    if !record.fsm.transition_to(WrapState::Expired, now) {
        panic_with_error!(e, ContractError::InvalidStateTransition);
    }

    e.storage().persistent().set(&wrap_key, &record);
    e.storage()
        .persistent()
        .extend_ttl(&wrap_key, TTL_ONE_YEAR, TTL_ONE_YEAR);

    e.events().publish(
        (symbol_short!("expire"), user, period),
        symbol_short!("expired"),
    );
}
