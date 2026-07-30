use crate::{ContractHealth, DataKey, WrapRecord};
use soroban_sdk::{Address, Bytes, BytesN, Env, String};

pub(crate) fn get_wrap(e: Env, user: Address, period: u64) -> Option<WrapRecord> {
    e.storage().persistent().get(&DataKey::Wrap(user, period))
}

pub(crate) fn get_mint_timestamp(e: Env, user: Address, period: u64) -> Option<u64> {
    let wrap: Option<WrapRecord> = e.storage().persistent().get(&DataKey::Wrap(user, period));
    wrap.map(|r| r.timestamp)
}

pub(crate) fn balance_of(e: Env, user: Address) -> i128 {
    let count_key = DataKey::WrapCount(user);
    e.storage()
        .persistent()
        .get::<_, u32>(&count_key)
        .unwrap_or(0) as i128
}

pub(crate) fn total_wrap_count(e: Env) -> u32 {
    e.storage()
        .persistent()
        .get(&DataKey::TotalWrapCount)
        .unwrap_or(0)
}

pub(crate) fn verify_data(e: Env, user: Address, period: u64, data: Bytes) -> bool {
    let wrap: Option<WrapRecord> = e.storage().persistent().get(&DataKey::Wrap(user, period));
    match wrap {
        Some(record) => {
            let computed_hash = e.crypto().sha256(&data);
            let computed_hash = BytesN::from_array(&e, &computed_hash.to_array());
            record.data_hash == computed_hash
        }
        None => false,
    }
}

pub(crate) fn get_latest_wrap(e: Env, user: Address) -> Option<WrapRecord> {
    let latest_key = DataKey::LatestPeriod(user.clone());
    let period: u64 = e.storage().persistent().get(&latest_key)?;
    e.storage().persistent().get(&DataKey::Wrap(user, period))
}

pub(crate) fn get_wraps(e: Env, user: Address, start: u32, limit: u32) -> soroban_sdk::Vec<WrapRecord> {
    let mut results = soroban_sdk::Vec::new(&e);
    let user_periods_key = DataKey::UserPeriods(user.clone());
    
    if let Some(periods) = e.storage().persistent().get::<_, soroban_sdk::Vec<u64>>(&user_periods_key) {
        let len = periods.len();
        if start < len {
            let end = core::cmp::min(start.saturating_add(limit), len);
            for i in start..end {
                if let Some(period) = periods.get(i) {
                    if let Some(wrap) = e.storage().persistent().get(&DataKey::Wrap(user.clone(), period)) {
                        results.push_back(wrap);
                    }
                }
            }
        }
    }
    
    results
}

pub(crate) fn health(e: Env) -> ContractHealth {
    let has_admin = e.storage().instance().has(&DataKey::Admin);
    let has_signing_key = e.storage().instance().has(&DataKey::AdminPubKey);

    ContractHealth {
        initialized: has_admin,
        has_admin,
        has_signing_key,
    }
}

pub(crate) fn get_admin(e: Env) -> Option<Address> {
    e.storage().instance().get(&DataKey::Admin)
}

pub(crate) fn total_revoked(e: Env) -> u64 {
    e.storage()
        .instance()
        .get::<_, u64>(&DataKey::TotalRevoked)
        .unwrap_or(0)
}

pub(crate) fn name(e: Env) -> String {
    e.storage()
        .instance()
        .get(&DataKey::Name)
        .unwrap_or_else(|| String::from_str(&e, "Stellar Wrap Registry"))
}

pub(crate) fn symbol(e: Env) -> String {
    e.storage()
        .instance()
        .get(&DataKey::Symbol)
        .unwrap_or_else(|| String::from_str(&e, "WRAP"))
}

pub(crate) fn decimals(_e: Env) -> u32 {
    0
}

pub(crate) fn contract_version(e: Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::ContractVersion)
        .unwrap_or(0)
}