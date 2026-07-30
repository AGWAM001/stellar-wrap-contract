use crate::DataKey;
use soroban_sdk::{Env, String};

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
