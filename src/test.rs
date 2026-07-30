#![cfg(test)]

extern crate std;

use super::*;
use crate::mint::CURRENT_PAYLOAD_VERSION;
use crate::test_utils::sign_payload;
use ed25519_dalek::SigningKey;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    Address, Bytes, BytesN, Env, String, Symbol, TryIntoVal,
};
use std::vec::Vec;

const STRESS_USER_COUNT: usize = 128;

#[test]
fn test_minting_flow() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let dummy_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &dummy_hash);
    client.mint_wrap(&user, &period, &archetype, &dummy_hash, &CURRENT_PAYLOAD_VERSION, &signature);
    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, dummy_hash);
}

#[test]
fn test_mint_emits_event() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[2u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let period = 202401u64;
    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);
    let events = env.events().all();
    let event_list = events.events();
    assert!(!event_list.is_empty(), "expected mint event");
}

#[test]
fn test_balance_of_and_count() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[3u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let archetype = symbol_short!("soroban");
    let hash = BytesN::from_array(&env, &[0u8; 32]);
    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, 202401, &archetype, &hash);
    client.mint_wrap(&user, &202401, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig1);
    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 202402, &archetype, &hash);
    client.mint_wrap(&user, &202402, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig2);
    assert_eq!(client.balance_of(&user), 2);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_initialize_twice_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);
    client.initialize(&admin, &pubkey);
}

#[test]
fn test_health_reflects_initialization_state() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let health = client.health();
    assert_eq!(health.initialized, false);
    assert_eq!(health.has_admin, false);
    assert_eq!(health.has_signing_key, false);
    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    let health = client.health();
    assert_eq!(health.initialized, true);
    assert_eq!(health.has_admin, true);
    assert_eq!(health.has_signing_key, true);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_duplicate_period_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[4u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;
    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);
}

#[test]
fn test_update_admin_success() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);
    env.mock_all_auths();
    client.update_admin(&new_admin);
    assert_eq!(client.get_admin().unwrap(), new_admin);
}

#[test]
fn test_token_metadata() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    assert_eq!(client.decimals(), 0);
    assert_eq!(client.name(), String::from_str(&env, "Stellar Wrap Registry"));
    assert_eq!(client.symbol(), String::from_str(&env, "WRAP"));
}

#[test]
fn test_verify_data_matching_hash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[5u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let data_json = Bytes::from_slice(&env, b"{\"score\":100,\"level\":\"gold\"}");
    let data_hash_raw = env.crypto().sha256(&data_json);
    let data_hash = BytesN::from_array(&env, &data_hash_raw.to_array());
    let archetype = symbol_short!("arch");
    let period = 202401u64;
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &data_hash);
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);
    assert!(client.verify_data(&user, &period, &data_json));
}

#[test]
fn test_verify_data_non_matching_hash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[6u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let original_data = Bytes::from_slice(&env, b"{\"score\":100}");
    let data_hash_raw = env.crypto().sha256(&original_data);
    let data_hash = BytesN::from_array(&env, &data_hash_raw.to_array());
    let archetype = symbol_short!("arch");
    let period = 202401u64;
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &data_hash);
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);
    let tampered_data = Bytes::from_slice(&env, b"{\"score\":999}");
    assert!(!client.verify_data(&user, &period, &tampered_data));
}

#[test]
fn test_verify_data_no_wrap_exists() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);
    let user = Address::generate(&env);
    let data = Bytes::from_slice(&env, b"anything");
    assert!(!client.verify_data(&user, &202401, &data));
}

#[test]
fn test_get_latest_wrap_returns_most_recent() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let archetype = symbol_short!("arch");
    let hash1 = BytesN::from_array(&env, &[10u8; 32]);
    let hash2 = BytesN::from_array(&env, &[20u8; 32]);
    let hash3 = BytesN::from_array(&env, &[30u8; 32]);
    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, 202402, &archetype, &hash1);
    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 202404, &archetype, &hash2);
    let sig3 = sign_payload(&env, &signing_key, &contract_id, &user, 202403, &archetype, &hash3);
    client.mint_wrap(&user, &202402, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);
    client.mint_wrap(&user, &202404, &archetype, &hash2, &CURRENT_PAYLOAD_VERSION, &sig2);
    client.mint_wrap(&user, &202403, &archetype, &hash3, &CURRENT_PAYLOAD_VERSION, &sig3);
    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, 202404);
    assert_eq!(latest.data_hash, hash2);
}

#[test]
fn test_get_latest_wrap_no_wraps() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);
    let user = Address::generate(&env);
    assert!(client.get_latest_wrap(&user).is_none());
}

#[test]
fn test_get_latest_wrap_single_mint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[8u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let hash = BytesN::from_array(&env, &[55u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202501u64;
    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);
    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, 202501);
    assert_eq!(latest.data_hash, hash);
}

#[test]
fn test_valid_period_boundaries() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let archetype = symbol_short!("arch");
    let lower_hash = BytesN::from_array(&env, &[60u8; 32]);
    let upper_hash = BytesN::from_array(&env, &[61u8; 32]);
    let lower_sig = sign_payload(&env, &signing_key, &contract_id, &user, 202401, &archetype, &lower_hash);
    let upper_sig = sign_payload(&env, &signing_key, &contract_id, &user, 210012, &archetype, &upper_hash);
    client.mint_wrap(&user, &202401, &archetype, &lower_hash, &CURRENT_PAYLOAD_VERSION, &lower_sig);
    client.mint_wrap(&user, &210012, &archetype, &upper_hash, &CURRENT_PAYLOAD_VERSION, &upper_sig);
    assert!(client.get_wrap(&user, &202401).is_some());
    assert!(client.get_wrap(&user, &210012).is_some());
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_invalid_period_zero_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[10u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let hash = BytesN::from_array(&env, &[70u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 0u64;
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_invalid_period_one_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[11u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let hash = BytesN::from_array(&env, &[71u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 1u64;
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_invalid_period_max_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[12u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let hash = BytesN::from_array(&env, &[72u8; 32]);
    let archetype = symbol_short!("arch");
    let period = u64::MAX;
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);
}

#[test]
fn test_stress_mint_100_plus_unique_users() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[13u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let archetype = symbol_short!("arch");
    let period = 202601u64;
    let mut users = Vec::with_capacity(STRESS_USER_COUNT);
    let mut cpu_samples = [0u64; STRESS_USER_COUNT];
    let mut mem_samples = [0u64; STRESS_USER_COUNT];
    for i in 0..STRESS_USER_COUNT {
        env.budget().reset_default();
        let user = Address::generate(&env);
        let hash = BytesN::from_array(&env, &[i as u8; 32]);
        let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
        client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);
        cpu_samples[i] = env.budget().cpu_instruction_cost();
        mem_samples[i] = env.budget().memory_bytes_cost();
        users.push(user);
    }
    assert!(cpu_samples[0] > 0);
    assert!(mem_samples[0] > 0);
    env.budget().reset_unlimited();
    for (i, user) in users.iter().enumerate() {
        let expected_hash = BytesN::from_array(&env, &[i as u8; 32]);
        let wrap = client.get_wrap(user, &period).unwrap();
        assert_eq!(wrap.period, period);
        assert_eq!(wrap.data_hash, expected_hash);
        assert_eq!(client.balance_of(user), 1);
    }
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_mint_wrap_before_init_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    env.mock_all_auths();
    let user = Address::generate(&env);
    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let archetype = symbol_short!("arch");
    let sig = BytesN::from_array(&env, &[0u8; 64]);
    client.mint_wrap(&user, &202401, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_update_admin_before_init_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    env.mock_all_auths();
    let new_admin = Address::generate(&env);
    client.update_admin(&new_admin);
}

#[test]
fn test_get_admin_before_init_returns_none() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    assert!(client.get_admin().is_none());
}

#[test]
fn test_get_admin_after_initialization() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);
    let stored_admin = client.get_admin();
    assert!(stored_admin.is_some());
    assert_eq!(stored_admin.unwrap(), admin);
}

#[test]
fn test_get_admin_after_initialization_returns_correct_admin() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[15u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    let fetched = client.get_admin().expect("admin should be set after init");
    assert_eq!(fetched, admin);
    let health = client.health();
    assert_eq!(health.initialized, true);
    assert_eq!(health.has_admin, true);
    assert_eq!(health.has_signing_key, true);
}

#[test]
fn test_get_admin_after_init_distinct_from_other_address() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let other = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[9u8; 32]);
    client.initialize(&admin, &pubkey);
    let stored = client.get_admin().unwrap();
    assert_eq!(stored, admin);
    assert_ne!(stored, other);
}

#[test]
fn test_migrate_applies_once_per_version() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);
    env.mock_all_auths();
    assert_eq!(client.migration_version(), 0);
    client.migrate(&1);
    assert_eq!(client.migration_version(), 1);
    client.migrate(&2);
    assert_eq!(client.migration_version(), 2);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_migrate_rejects_replay() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);
    env.mock_all_auths();
    client.migrate(&1);
    client.migrate(&1);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_migrate_before_init_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    env.mock_all_auths();
    client.migrate(&1);
}

#[test]
fn test_get_mint_timestamp_exists() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[14u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let dummy_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &dummy_hash);
    client.mint_wrap(&user, &period, &archetype, &dummy_hash, &CURRENT_PAYLOAD_VERSION, &signature);
    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(client.get_mint_timestamp(&user, &period), Some(wrap.timestamp));
}

#[test]
fn test_get_mint_timestamp_missing() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let user = Address::generate(&env);
    let period = 202401u64;
    assert_eq!(client.get_mint_timestamp(&user, &period), None);
}

#[test]
fn test_get_wraps_paginated() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[20u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let archetype = symbol_short!("arch");
    for i in 0..3 {
        let period = 202401 + i as u64;
        let hash = BytesN::from_array(&env, &[i as u8; 32]);
        let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
        client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);
    }
    let wraps = client.get_wraps(&user, &0, &10);
    assert_eq!(wraps.len(), 3);
}

#[test]
fn test_revoke_wrap_flow() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[21u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;
    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);
    assert!(client.get_wrap(&user, &period).is_some());
    let reason = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason);
    assert!(client.get_wrap(&user, &period).is_none());
}

#[test]
fn test_fsm_valid_state_transitions() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[22u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let hash = BytesN::from_array(&env, &[2u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202402u64;
    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);
    client.transition_wrap_state(&user, &period, &WrapState::Archived);
    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.fsm.state, WrapState::Archived);
}

#[test]
fn test_get_wrap_returns_none_before_initialization() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let user = Address::generate(&env);
    assert!(client.get_wrap(&user, &202401).is_none());
}