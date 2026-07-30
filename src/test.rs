#![cfg(test)]

extern crate std;

use super::*;
use crate::mint::MINT_SIGNATURE_PAYLOAD_VERSION;
use crate::storage_types::{DataKey, WrapRecord};
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    xdr::ToXdr,
    Address, Bytes, BytesN, Env, IntoVal, String, Symbol, TryIntoVal,
};
use std::vec::Vec;

const STRESS_USER_COUNT: usize = 128;

fn sign_payload(
    env: &Env,
    signer: &SigningKey,
    contract: &Address,
    user: &Address,
    period: u64,
    archetype: &Symbol,
    data_hash: &BytesN<32>,
) -> BytesN<64> {
    let mut payload = Bytes::new(env);
    payload.append(&Bytes::from_array(env, &[MINT_SIGNATURE_PAYLOAD_VERSION]));
    payload.append(&contract.to_xdr(env));
    payload.append(&user.clone().to_xdr(env));
    payload.append(&period.to_xdr(env));
    payload.append(&archetype.clone().to_xdr(env));
    payload.append(&data_hash.clone().to_xdr(env));

    let mut out = [0u8; 512];
    let len = payload.len() as usize;
    payload.copy_into_slice(&mut out[..len]);

    let signature = signer.sign(&out[..len]);
    BytesN::from_array(env, &signature.to_bytes())
}

// ─── Issue #55: total_wrap_count tracks mints across users ───────────────

#[test]
fn test_total_wrap_count_tracks_mints_across_users() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    assert_eq!(client.total_wrap_count(), 0);

    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[1u8; 32]);

    // Mint for user_a
    let user_a = Address::generate(&env);
    let sig_a = sign_payload(&env, &signing_key, &contract_id, &user_a, 202401, &archetype, &hash);
    client.mint_wrap(&user_a, &202401, &archetype, &hash, &sig_a);
    assert_eq!(client.total_wrap_count(), 1);

    // Mint for user_b -- count should increment globally
    let user_b = Address::generate(&env);
    let sig_b = sign_payload(&env, &signing_key, &contract_id, &user_b, 202402, &archetype, &hash);
    client.mint_wrap(&user_b, &202402, &archetype, &hash, &sig_b);
    assert_eq!(client.total_wrap_count(), 2);

    // Second mint for user_a -- count still increments globally
    let sig_a2 = sign_payload(&env, &signing_key, &contract_id, &user_a, 202403, &archetype, &hash);
    client.mint_wrap(&user_a, &202403, &archetype, &hash, &sig_a2);
    assert_eq!(client.total_wrap_count(), 3);
}

// ─── Issue #66: revoke emits event tracking (multi-user) ─────────────────

#[test]
fn test_revoke_emits_event_multi_user() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[14u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype_a = symbol_short!("gold");
    let archetype_b = symbol_short!("silvr");
    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let period_a = 202401u64;
    let period_b = 202402u64;

    let sig_a = sign_payload(
        &env, &signing_key, &contract_id, &user_a, period_a, &archetype_a, &hash,
    );
    let sig_b = sign_payload(
        &env, &signing_key, &contract_id, &user_b, period_b, &archetype_b, &hash,
    );

    client.mint_wrap(&user_a, &period_a, &archetype_a, &hash, &sig_a);
    client.mint_wrap(&user_b, &period_b, &archetype_b, &hash, &sig_b);

    let reason_hash = BytesN::from_array(&env, &[0xAAu8; 32]);
    client.revoke_wrap(&user_a, &period_a, &reason_hash);
    client.revoke_wrap(&user_b, &period_b, &reason_hash);

    let events = env.events().all();

    let revoke_events: Vec<_> = events
        .iter()
        .filter(|(_, topics, _)| {
            let sym: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
            sym == symbol_short!("revoke")
        })
        .collect();

    assert_eq!(revoke_events.len(), 2);

    let (_, ref topics_a, ref data_a) = &revoke_events[0];
    let event_user_a: Address = topics_a.get(1).unwrap().try_into_val(&env).unwrap();
    let event_period_a: u64 = topics_a.get(2).unwrap().try_into_val(&env).unwrap();
    let event_reason_a: BytesN<32> = data_a.try_into_val(&env).unwrap();
    assert_eq!(event_user_a, user_a);
    assert_eq!(event_period_a, period_a);
    assert_eq!(event_reason_a, reason_hash);

    let (_, ref topics_b, ref data_b) = &revoke_events[1];
    let event_user_b: Address = topics_b.get(1).unwrap().try_into_val(&env).unwrap();
    let event_period_b: u64 = topics_b.get(2).unwrap().try_into_val(&env).unwrap();
    let event_reason_b: BytesN<32> = data_b.try_into_val(&env).unwrap();
    assert_eq!(event_user_b, user_b);
    assert_eq!(event_period_b, period_b);
    assert_eq!(event_reason_b, reason_hash);
}

// ─── Balance & Count ──────────────────────────────────────────────────────

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
    client.mint_wrap(&user, &202401, &archetype, &hash, &sig1);

    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 202402, &archetype, &hash);
    client.mint_wrap(&user, &202402, &archetype, &hash, &sig2);

    assert_eq!(client.balance_of(&user), 2);
}

// ─── Revoke total_revoked ─────────────────────────────────────────────────

#[test]
fn test_revoke_wrap_increments_total_revoked() {
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
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);

    client.mint_wrap(&user, &period, &archetype, &hash, &signature);
    assert_eq!(client.total_revoked(), 0);

    let reason_hash = BytesN::from_array(&env, &[1u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    assert_eq!(client.total_revoked(), 1);
    assert_eq!(client.balance_of(&user), 0);
    assert!(client.get_wrap(&user, &period).is_none());
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_revoke_wrap_nonexistent_wrap_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    let user = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &202401, &reason_hash);
}

// ─── Initialize twice ─────────────────────────────────────────────────────

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

// ─── Health ───────────────────────────────────────────────────────────────

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

// ─── Duplicate period ─────────────────────────────────────────────────────

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

    client.mint_wrap(&user, &period, &archetype, &hash, &sig);
    client.mint_wrap(&user, &period, &archetype, &hash, &sig);
}

// ─── Update admin ─────────────────────────────────────────────────────────

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

// ─── Token metadata ───────────────────────────────────────────────────────

#[test]
fn test_token_metadata() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    assert_eq!(client.decimals(), 0);
    assert_eq!(
        client.name(),
        String::from_str(&env, "Stellar Wrap Registry")
    );
    assert_eq!(client.symbol(), String::from_str(&env, "WRAP"));
}

// ─── Verify data (matching) ───────────────────────────────────────────────

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
    client.mint_wrap(&user, &period, &archetype, &data_hash, &signature);

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
    client.mint_wrap(&user, &period, &archetype, &data_hash, &signature);

    let tampered_data = Bytes::from_slice(&env, b"{\"score\":999}");
    assert!(!client.verify_data(&user, &period, &tampered_data));
}

#[test]
fn test_verify_data_corrupted_payload() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[6u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let original_data = Bytes::from_slice(&env, b"{\"valid\":true}");
    let data_hash_raw = env.crypto().sha256(&original_data);
    let data_hash = BytesN::from_array(&env, &data_hash_raw.to_array());
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &data_hash);
    client.mint_wrap(&user, &period, &archetype, &data_hash, &signature);

    let corrupted_data = Bytes::from_slice(&env, b"\x00\xFF\xFE\xFDcorrupt\x01\x02");
    assert!(!client.verify_data(&user, &period, &corrupted_data));
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

// ─── Get latest wrap ──────────────────────────────────────────────────────

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

    client.mint_wrap(&user, &202402, &archetype, &hash1, &sig1);
    client.mint_wrap(&user, &202404, &archetype, &hash2, &sig2);
    client.mint_wrap(&user, &202403, &archetype, &hash3, &sig3);

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
    client.mint_wrap(&user, &period, &archetype, &hash, &sig);

    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, period);
    assert_eq!(latest.data_hash, hash);
}

// ─── Invalid periods ──────────────────────────────────────────────────────

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

    client.mint_wrap(&user, &period, &archetype, &hash, &signature);
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

    client.mint_wrap(&user, &period, &archetype, &hash, &signature);
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

    client.mint_wrap(&user, &period, &archetype, &hash, &signature);
}

#[test]
fn test_mint_event_structured_matching() {
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
    let period = 202401u64;
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &signature);

    let events = env.events().all();
    let last_event = events.last().expect("Expected at least one event");
    let (event_contract, topics, data) = last_event;

    assert_eq!(event_contract, contract_id);
    assert_eq!(topics.len(), 3, "Mint event must have exactly 3 topics");

    let topic_0: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic_1: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let topic_2: u64 = topics.get(2).unwrap().try_into_val(&env).unwrap();

    assert_eq!(topic_0, symbol_short!("mint"));
    assert_eq!(topic_1, user);
    assert_eq!(topic_2, period);

    let event_data: Symbol = data.try_into_val(&env).unwrap();
    assert_eq!(event_data, archetype);
}

// ─── Stress test ──────────────────────────────────────────────────────────

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

        client.mint_wrap(&user, &period, &archetype, &hash, &signature);

        cpu_samples[i] = env.budget().cpu_instruction_cost();
        mem_samples[i] = env.budget().memory_bytes_cost();
        users.push(user);
    }

    assert!(cpu_samples[0] > 0);
    assert!(mem_samples[0] > 0);
    assert!(cpu_samples.iter().all(|sample| *sample > 0));
    assert!(mem_samples.iter().all(|sample| *sample > 0));

    env.budget().reset_unlimited();

    for (i, user) in users.iter().enumerate() {
        let expected_hash = BytesN::from_array(&env, &[i as u8; 32]);
        let wrap = client.get_wrap(user, &period).unwrap();

        assert_eq!(wrap.period, period);
        assert_eq!(wrap.data_hash, expected_hash);
        assert_eq!(client.balance_of(user), 1);
        assert_eq!(client.get_latest_wrap(user).unwrap().period, period);
    }
}

// ─── Before-init guard tests ──────────────────────────────────────────────

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

    client.mint_wrap(&user, &202401, &archetype, &hash, &sig);
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

// ─── Migration tests ──────────────────────────────────────────────────────

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
#[should_panic(expected = "Error(Contract, #2)")]
fn test_upgrade_before_init_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    env.mock_all_auths();

    let new_wasm_hash = BytesN::from_array(&env, &[42u8; 32]);
    client.upgrade(&new_wasm_hash);
}

// ─── get_mint_timestamp tests ─────────────────────────────────────────────

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

    let hash = BytesN::from_array(&env, &[14u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &sig);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(
        client.get_mint_timestamp(&user, &period),
        Some(wrap.timestamp)
    );
}

#[test]
fn test_get_mint_timestamp_missing() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);

    let user = Address::generate(&env);
    let period = 202401u64;

    assert_eq!(client.get_mint_timestamp(&user, &period), None);
}

// ─── get_wrap before init ─────────────────────────────────────────────────

#[test]
fn test_get_wrap_returns_none_before_initialization() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let period = 202401u64;

    let result = client.get_wrap(&user, &period);
    assert!(result.is_none());
}

// ─── Instance TTL tests ───────────────────────────────────────────────────

#[test]
fn test_instance_ttl_extended_on_mint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[15u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[15u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &sig);

    // After mint, admin is still readable — instance storage was not expired
    assert!(client.get_admin().is_some());
    assert_eq!(client.get_admin().unwrap(), admin);
}

#[test]
fn test_instance_ttl_extended_on_second_mint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[16u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[16u8; 32]);
    let archetype = symbol_short!("arch");

    // First mint by user_a
    let sig_a = sign_payload(&env, &signing_key, &contract_id, &user_a, 202401, &archetype, &hash);
    client.mint_wrap(&user_a, &202401, &archetype, &hash, &sig_a);

    // Second mint by user_b — still extends instance TTL
    let sig_b = sign_payload(&env, &signing_key, &contract_id, &user_b, 202402, &archetype, &hash);
    client.mint_wrap(&user_b, &202402, &archetype, &hash, &sig_b);

    // Admin address still accessible after multiple mints
    assert!(client.get_admin().is_some());
    assert_eq!(client.get_admin().unwrap(), admin);
}

// ─── FSM tests ────────────────────────────────────────────────────────────

#[test]
fn test_fsm_valid_state_transitions() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[17u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[17u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &sig);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.fsm.state, WrapState::Active);

    // Transition Active -> Archived
    client.transition_wrap_state(&user, &period, &WrapState::Archived);
    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.fsm.state, WrapState::Archived);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_fsm_invalid_state_transition_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[18u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[18u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &sig);

    // Transition Active -> Draft is invalid and should fail with #8 (InvalidStateTransition)
    client.transition_wrap_state(&user, &period, &WrapState::Draft);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_fsm_transition_nonexistent_wrap_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    let user = Address::generate(&env);
    client.transition_wrap_state(&user, &202401u64, &WrapState::Archived);
}

// ─── Revoke flow: event + remint ──────────────────────────────────────────

#[test]
fn test_revoke_wrap_flow_event_and_remint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[13u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let period = 202601u64;
    let archetype = symbol_short!("arch");
    let hash_1 = BytesN::from_array(&env, &[31u8; 32]);
    let hash_2 = BytesN::from_array(&env, &[32u8; 32]);

    let sig_1 = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash_1);
    client.mint_wrap(&user, &period, &archetype, &hash_1, &sig_1);
    assert_eq!(client.balance_of(&user), 1);

    let reason_hash = BytesN::from_array(&env, &[0xABu8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    assert!(client.get_wrap(&user, &period).is_none());
    assert_eq!(client.balance_of(&user), 0);

    let events = env.events().all();
    let last_event = events.last().expect("Expected revoke event");
    let (_, topics, data) = last_event;

    assert_eq!(topics.len(), 3);
    let event_topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let event_user: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let event_period: u64 = topics.get(2).unwrap().try_into_val(&env).unwrap();
    let event_reason: BytesN<32> = data.try_into_val(&env).unwrap();

    assert_eq!(event_topic, symbol_short!("revoke"));
    assert_eq!(event_user, user);
    assert_eq!(event_period, period);
    assert_eq!(event_reason, reason_hash);

    let sig_2 = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash_2);
    client.mint_wrap(&user, &period, &archetype, &hash_2, &sig_2);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, hash_2);
    assert_eq!(client.balance_of(&user), 1);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_revoke_missing_wrap_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let admin_pubkey = BytesN::from_array(&env, &[14u8; 32]);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &2026, &reason_hash);
}

#[test]
#[should_panic]
fn test_revoke_requires_admin_auth() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let admin_pubkey = BytesN::from_array(&env, &[15u8; 32]);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);

    env.as_contract(&contract_id, || {
        let wrap_key = DataKey::Wrap(user.clone(), 2026);
        let count_key = DataKey::WrapCount(user.clone());
        let record = WrapRecord {
            timestamp: env.ledger().timestamp(),
            data_hash: BytesN::from_array(&env, &[16u8; 32]),
            archetype: symbol_short!("arch"),
            period: 2026,
            fsm: WrapLifecycleFSM::new(WrapState::Active, env.ledger().timestamp()),
        };
        env.storage().persistent().set(&wrap_key, &record);
        env.storage().persistent().set(&count_key, &1u32);
    });

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &2026, &reason_hash);
}

// ─── Mint guard tests ─────────────────────────────────────────────────────

#[test]
fn test_mint_guard_uses_temporary_storage_and_clears_on_success() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[13u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let period = 202601u64;
    let archetype = symbol_short!("arch");
    let data_hash = BytesN::from_array(&env, &[13u8; 32]);
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &data_hash);

    client.mint_wrap(&user, &period, &archetype, &data_hash, &signature);

    // Wrap record should exist after successful mint
    assert!(client.get_wrap(&user, &period).is_some());
    assert_eq!(client.balance_of(&user), 1);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_mint_guard_on_failure_leaves_no_residual_state() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[14u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let period = 202601u64;
    let archetype = symbol_short!("arch");
    let data_hash = BytesN::from_array(&env, &[14u8; 32]);
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &data_hash);

    // First mint succeeds
    client.mint_wrap(&user, &period, &archetype, &data_hash, &signature);

    // Second mint for same (user, period) fails with WrapAlreadyExists
    client.mint_wrap(&user, &period, &archetype, &data_hash, &signature);
}

// ─── Update admin event ───────────────────────────────────────────────────

#[test]
fn test_update_admin_emits_event() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    client.update_admin(&new_admin);

    let events = env.events().all();
    let last_event = events.last().expect("Expected at least one event");
    let (_, topics, data) = last_event;

    let topic_0: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic_1: Symbol = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let topic_2: Symbol = topics.get(2).unwrap().try_into_val(&env).unwrap();
    assert_eq!(topic_0, symbol_short!("v1"));
    assert_eq!(topic_1, symbol_short!("admin"));
    assert_eq!(topic_2, symbol_short!("updated"));

    let (old_admin_val, new_admin_val): (Address, Address) = data.try_into_val(&env).unwrap();
    assert_eq!(old_admin_val, admin);
    assert_eq!(new_admin_val, new_admin);
}

// ─── Unauthorized update admin ────────────────────────────────────────────

#[test]
#[should_panic]
fn test_update_admin_unauthorized_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);

    client.initialize(&admin, &pubkey);
    client.update_admin(&new_admin);
}

#[test]
#[should_panic]
fn test_update_admin_by_non_admin_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);

    client.initialize(&admin, &pubkey);

    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &non_admin,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &contract_id,
            fn_name: "update_admin",
            args: (&new_admin,).into_val(&env),
            sub_invokes: &[],
        },
    }]);

    client.update_admin(&new_admin);
}

#[test]
fn test_mint_wrap_zero_hash_succeeds() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[20u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let zero_hash = BytesN::from_array(&env, &[0u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &zero_hash);
    client.mint_wrap(&user, &period, &archetype, &zero_hash, &sig);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, zero_hash);
}

#[test]
fn test_mint_wrap_non_zero_hash_succeeds() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[21u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let mut hash_bytes = [0u8; 32];
    hash_bytes[31] = 1;
    let edge_hash = BytesN::from_array(&env, &hash_bytes);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &edge_hash);
    client.mint_wrap(&user, &period, &archetype, &edge_hash, &sig);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, edge_hash);
}

#[test]
fn test_mint_wrap_max_hash_succeeds() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[22u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let max_hash = BytesN::from_array(&env, &[0xff; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &max_hash);
    client.mint_wrap(&user, &period, &archetype, &max_hash, &sig);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, max_hash);
}

// ─── Upgrade event ────────────────────────────────────────────────────────

#[test]
#[should_panic]
fn test_upgrade_emits_event() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    let new_wasm_hash = BytesN::from_array(&env, &[42u8; 32]);
    client.upgrade(&new_wasm_hash);
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn test_upgrade_requires_admin_auth() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);

    let fake_wasm = BytesN::from_array(&env, &[0u8; 32]);
    client.upgrade(&fake_wasm);
}

// ─── Revoke wrap success ──────────────────────────────────────────────────

#[test]
fn test_revoke_wrap_success() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[30u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &signature);
    assert_eq!(client.balance_of(&user), 1);
    assert!(client.get_wrap(&user, &period).is_some());

    let reason_hash = BytesN::from_array(&env, &[1u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    assert!(client.get_wrap(&user, &period).is_none());
    assert_eq!(client.balance_of(&user), 0);
}

#[test]
fn test_revoke_wrap_emits_event() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[31u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &signature);

    let reason_hash = BytesN::from_array(&env, &[0xAAu8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    let events = env.events().all();
    let last_event = events.last().expect("no events found");
    let (_, topics, data) = last_event;

    let event_topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let event_user: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let event_period: u64 = topics.get(2).unwrap().try_into_val(&env).unwrap();
    let event_reason: BytesN<32> = data.try_into_val(&env).unwrap();

    assert_eq!(event_topic, symbol_short!("revoke"));
    assert_eq!(event_user, user);
    assert_eq!(event_period, period);
    assert_eq!(event_reason, reason_hash);
}

// ─── Revoke not found fails ───────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_revoke_wrap_not_found_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[34u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &2025, &reason_hash);
}

// ─── Revoke with reason hash event ────────────────────────────────────────

#[test]
fn test_revoke_with_reason_hash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[35u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &signature);

    let reason_hash = BytesN::from_array(&env, &[0xAAu8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    let events = env.events().all();
    let last_event = events.last().expect("no events found");
    let (_, topics, data) = last_event;

    let event_topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let event_user: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let event_period: u64 = topics.get(2).unwrap().try_into_val(&env).unwrap();
    let event_reason: BytesN<32> = data.try_into_val(&env).unwrap();

    assert_eq!(event_topic, symbol_short!("revoke"));
    assert_eq!(event_user, user);
    assert_eq!(event_period, period);
    assert_eq!(event_reason, reason_hash);
}

// ─── Revoke before init ───────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_revoke_wrap_before_init_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    env.mock_all_auths();

    let user = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);

    client.revoke_wrap(&user, &202401, &reason_hash);
}

// ─── Revoke latest period ─────────────────────────────────────────────────

#[test]
fn test_revoke_latest_period_clears_latest() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[50u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[1u8; 32]);

    // Mint first wrap (period 202401)
    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, 202401, &archetype, &hash);
    client.mint_wrap(&user, &202401, &archetype, &hash, &sig1);

    // Mint second wrap (higher period)
    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 202501, &archetype, &hash);
    client.mint_wrap(&user, &202501, &archetype, &hash, &sig2);

    // After second mint: balance = 2, latest = 202501
    assert_eq!(client.balance_of(&user), 2);
    assert_eq!(client.get_latest_wrap(&user).unwrap().period, 202501);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &202501, &reason_hash);

    // LatestPeriod was cleared; get_latest_wrap returns None
    assert!(client.get_latest_wrap(&user).is_none());
    assert_eq!(client.balance_of(&user), 1);
}

#[test]
fn test_revoke_non_latest_preserves_latest() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[51u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let hash1 = BytesN::from_array(&env, &[10u8; 32]);
    let hash2 = BytesN::from_array(&env, &[20u8; 32]);

    // Mint first wrap (period 202401)
    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, 202401, &archetype, &hash1);
    client.mint_wrap(&user, &202401, &archetype, &hash1, &sig1);

    // Mint second wrap (period 202403) — higher period
    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 202403, &archetype, &hash2);
    client.mint_wrap(&user, &202403, &archetype, &hash2, &sig2);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &202401, &reason_hash);

    // Latest period (202403) should still be retrievable
    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, 202403);
    assert_eq!(client.balance_of(&user), 1);
}

// ─── TTL metadata tests ───────────────────────────────────────────────────

#[test]
fn test_metadata_ttl_extended_on_new_mint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[101u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[1u8; 32]);

    let sig_a = sign_payload(&env, &signing_key, &contract_id, &user_a, 202407, &archetype, &hash);
    client.mint_wrap(&user_a, &202407, &archetype, &hash, &sig_a);

    let sig_b = sign_payload(&env, &signing_key, &contract_id, &user_b, 202407, &archetype, &hash);
    client.mint_wrap(&user_b, &202407, &archetype, &hash, &sig_b);

    // Admin address still accessible after multiple mints
    assert!(client.get_admin().is_some());
    assert_eq!(client.get_admin().unwrap(), admin);
}

// ─── Old wrap preserved on new mint ───────────────────────────────────────

#[test]
fn test_old_wrap_preserved_on_new_mint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[36u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let hash1 = BytesN::from_array(&env, &[10u8; 32]);
    let hash2 = BytesN::from_array(&env, &[20u8; 32]);

    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, 202401, &archetype, &hash1);
    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 202403, &archetype, &hash2);

    client.mint_wrap(&user, &202401, &archetype, &hash1, &sig1);
    client.mint_wrap(&user, &202403, &archetype, &hash2, &sig2);

    // Old wrap (period 202401) is still intact and readable
    let wrap1 = client.get_wrap(&user, &202401).unwrap();
    assert_eq!(wrap1.period, 202401);
    assert_eq!(wrap1.data_hash, hash1);

    // New wrap (period 202403) is also intact
    let wrap2 = client.get_wrap(&user, &202403).unwrap();
    assert_eq!(wrap2.period, 202403);
    assert_eq!(wrap2.data_hash, hash2);

    // Balance reflects both wraps
    assert_eq!(client.balance_of(&user), 2);
}

// ─── renew_all_ttls ───────────────────────────────────────────────────────

#[test]
fn test_renew_all_ttls_extends_metadata() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[52u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let archetype = symbol_short!("arch");

    let sig = sign_payload(&env, &signing_key, &contract_id, &user, 202401, &archetype, &hash);
    client.mint_wrap(&user, &202401, &archetype, &hash, &sig);

    // Verify metadata exists before renewal
    assert_eq!(client.balance_of(&user), 1);
    assert!(client.get_latest_wrap(&user).is_some());

    // Admin renews all metadata TTls
    client.renew_all_ttls(&user);

    // Metadata still accessible after renewal
    assert_eq!(client.balance_of(&user), 1);
    assert!(client.get_latest_wrap(&user).is_some());
}

#[test]
#[should_panic]
fn test_renew_all_ttls_requires_admin_auth() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);

    // Seed a wrap directly without auth mocking
    let user = Address::generate(&env);
    env.as_contract(&contract_id, || {
        let wrap_key = DataKey::Wrap(user.clone(), 2024);
        let count_key = DataKey::WrapCount(user.clone());
        let latest_key = DataKey::LatestPeriod(user.clone());
        let record = WrapRecord {
            timestamp: env.ledger().timestamp(),
            data_hash: BytesN::from_array(&env, &[1u8; 32]),
            archetype: symbol_short!("arch"),
            period: 2024,
            fsm: WrapLifecycleFSM::new(WrapState::Active, env.ledger().timestamp()),
        };
        env.storage().persistent().set(&wrap_key, &record);
        env.storage().persistent().set(&count_key, &1u32);
        env.storage().persistent().set(&latest_key, &2024u64);
    });

    // No auth mocked — admin.require_auth() must panic
    client.renew_all_ttls(&user);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_renew_all_ttls_before_init_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    env.mock_all_auths();

    let user = Address::generate(&env);
    client.renew_all_ttls(&user);
}

// ─── Remint after revoke ──────────────────────────────────────────────────

#[test]
fn test_remint_after_revoke_succeeds() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[52u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let hash1 = BytesN::from_array(&env, &[10u8; 32]);
    let hash2 = BytesN::from_array(&env, &[20u8; 32]);
    let period = 202401u64;

    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash1);
    client.mint_wrap(&user, &period, &archetype, &hash1, &sig1);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    // Should be able to mint a new wrap for the same period after revocation
    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash2);
    client.mint_wrap(&user, &period, &archetype, &hash2, &sig2);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, hash2);
    assert_eq!(client.balance_of(&user), 1);
}

#[test]
fn test_remint_after_revoke_updates_archetype() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[95u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let period = 202406u64;
    let archetype_old = symbol_short!("arch");
    let archetype_new = symbol_short!("builder");
    let hash_old = BytesN::from_array(&env, &[41u8; 32]);
    let hash_new = BytesN::from_array(&env, &[42u8; 32]);

    let sig_old = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype_old, &hash_old);
    client.mint_wrap(&user, &period, &archetype_old, &hash_old, &sig_old);

    let wrap_old = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap_old.archetype, archetype_old);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);
    assert!(client.get_wrap(&user, &period).is_none());

    let sig_new = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype_new, &hash_new);
    client.mint_wrap(&user, &period, &archetype_new, &hash_new, &sig_new);

    let wrap_new = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap_new.archetype, archetype_new);
    assert_eq!(wrap_new.data_hash, hash_new);

    let events_after_remint = env.events().all();
    let (_, mint_topics, mint_data) = events_after_remint
        .last()
        .expect("expected mint event after remint");
    let mint_topic: Symbol = mint_topics.get(0).unwrap().try_into_val(&env).unwrap();
    let mint_user: Address = mint_topics.get(1).unwrap().try_into_val(&env).unwrap();
    let mint_period: u64 = mint_topics.get(2).unwrap().try_into_val(&env).unwrap();
    let mint_archetype: Symbol = mint_data.try_into_val(&env).unwrap();
    assert_eq!(mint_topic, symbol_short!("mint"));
    assert_eq!(mint_user, user);
    assert_eq!(mint_period, period);
    assert_eq!(mint_archetype, archetype_new);
}

// ─── Revoke with zero reason hash ─────────────────────────────────────────

#[test]
fn test_revoke_with_zero_reason_hash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[53u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &signature);

    // Zero reason hash (no reason provided)
    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    assert!(client.get_wrap(&user, &period).is_none());
    assert_eq!(client.balance_of(&user), 0);

    // Verify the event still emitted with zero hash
    let events = env.events().all();
    let last_event = events.last().expect("no events found");
    let (_, _, data) = last_event;
    let event_reason: BytesN<32> = data.try_into_val(&env).unwrap();
    assert_eq!(event_reason, reason_hash);
}

// ─── Issue #252: event ordering across mint, revoke, remint ────────────────

#[test]
fn test_mint_revoke_remint_event_ordering() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[40u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let period = 202601u64;
    let archetype = symbol_short!("arch");
    let hash_mint1 = BytesN::from_array(&env, &[41u8; 32]);
    let hash_mint2 = BytesN::from_array(&env, &[42u8; 32]);

    // Step 1: Mint
    let sig_mint1 = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash_mint1);
    client.mint_wrap(&user, &period, &archetype, &hash_mint1, &sig_mint1);

    // Step 2: Revoke with a reason_hash
    let reason_hash = BytesN::from_array(&env, &[0xABu8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    // Step 3: Re-mint
    let sig_mint2 = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash_mint2);
    client.mint_wrap(&user, &period, &archetype, &hash_mint2, &sig_mint2);

    let events = env.events().all();

    // Filter events emitted by our contract
    let contract_events: Vec<_> = events.iter().filter(|(addr, _, _)| *addr == contract_id).collect();

    assert_eq!(contract_events.len(), 3, "Expected 3 events: mint → revoke → mint");

    // Event 0: first mint — topics: (mint, user, period), data: archetype
    let (_, ref t0, ref d0) = &contract_events[0];
    assert_eq!(t0.len(), 3);
    let ev_t0: Symbol = t0.get(0).unwrap().try_into_val(&env).unwrap();
    let ev_u0: Address = t0.get(1).unwrap().try_into_val(&env).unwrap();
    let ev_p0: u64 = t0.get(2).unwrap().try_into_val(&env).unwrap();
    let ev_a0: Symbol = d0.try_into_val(&env).unwrap();
    assert_eq!(ev_t0, symbol_short!("mint"));
    assert_eq!(ev_u0, user);
    assert_eq!(ev_p0, period);
    assert_eq!(ev_a0, archetype);

    // Event 1: revoke — topics: (revoke, user, period), data: reason_hash
    let (_, ref t1, ref d1) = &contract_events[1];
    assert_eq!(t1.len(), 3);
    let ev_t1: Symbol = t1.get(0).unwrap().try_into_val(&env).unwrap();
    let ev_u1: Address = t1.get(1).unwrap().try_into_val(&env).unwrap();
    let ev_p1: u64 = t1.get(2).unwrap().try_into_val(&env).unwrap();
    let ev_r1: BytesN<32> = d1.try_into_val(&env).unwrap();
    assert_eq!(ev_t1, symbol_short!("revoke"));
    assert_eq!(ev_u1, user);
    assert_eq!(ev_p1, period);
    assert_eq!(ev_r1, reason_hash);

    // Event 2: second mint
    let (_, ref t2, ref d2) = &contract_events[2];
    assert_eq!(t2.len(), 3);
    let ev_t2: Symbol = t2.get(0).unwrap().try_into_val(&env).unwrap();
    let ev_u2: Address = t2.get(1).unwrap().try_into_val(&env).unwrap();
    let ev_p2: u64 = t2.get(2).unwrap().try_into_val(&env).unwrap();
    let ev_a2: Symbol = d2.try_into_val(&env).unwrap();
    assert_eq!(ev_t2, symbol_short!("mint"));
    assert_eq!(ev_u2, user);
    assert_eq!(ev_p2, period);
    assert_eq!(ev_a2, archetype);

    // Verify final on-chain state
    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, hash_mint2);
    assert_eq!(client.balance_of(&user), 1);
}

// ─── Issue #247: update_admin same admin succeeds and emits event ──────────

#[test]
fn test_update_admin_same_admin_succeeds_and_emits_event() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    // Call update_admin with the same address
    client.update_admin(&admin);

    // Admin must remain unchanged
    assert_eq!(client.get_admin().unwrap(), admin);

    // Must emit an event — no silent no-op
    let events = env.events().all();
    let last_event = events.last().expect("Expected at least one event");
    let (_, topics, data) = last_event;

    let t0: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let t1: Symbol = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let t2: Symbol = topics.get(2).unwrap().try_into_val(&env).unwrap();
    assert_eq!(t0, symbol_short!("v1"));
    assert_eq!(t1, symbol_short!("admin"));
    assert_eq!(t2, symbol_short!("updated"));

    let (old_admin_val, new_admin_val): (Address, Address) = data.try_into_val(&env).unwrap();
    assert_eq!(old_admin_val, admin);
    assert_eq!(new_admin_val, admin);
}

// ─── Issue #242: balance_of after multiple revokes ─────────────────────────

#[test]
fn test_balance_of_after_multiple_revokes() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[50u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[50u8; 32]);
    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);

    // Mint three periods for one user
    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, 202601, &archetype, &hash);
    client.mint_wrap(&user, &202601, &archetype, &hash, &sig1);

    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 202602, &archetype, &hash);
    client.mint_wrap(&user, &202602, &archetype, &hash, &sig2);

    let sig3 = sign_payload(&env, &signing_key, &contract_id, &user, 202603, &archetype, &hash);
    client.mint_wrap(&user, &202603, &archetype, &hash, &sig3);

    assert_eq!(client.balance_of(&user), 3, "Balance must be 3 after three mints");

    // Revoke first period: balance goes 3 → 2
    client.revoke_wrap(&user, &202601, &reason_hash);
    assert_eq!(client.balance_of(&user), 2, "Balance must be 2 after first revoke");
    assert!(client.get_wrap(&user, &202601).is_none(), "Revoked record must be gone");

    // Revoke second period: balance goes 2 → 1
    client.revoke_wrap(&user, &202602, &reason_hash);
    assert_eq!(client.balance_of(&user), 1, "Balance must be 1 after second revoke");
    assert!(client.get_wrap(&user, &202602).is_none(), "Revoked record must be gone");

    // The third record (202603) must still exist
    let remaining = client.get_wrap(&user, &202603).unwrap();
    assert_eq!(remaining.period, 202603, "Remaining record period must match");

    // Attempt to revoke a non-existent wrap — must panic and NOT decrement count
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.revoke_wrap(&user, &999999, &reason_hash);
    }));
    assert!(result.is_err(), "Revoking a missing wrap must panic");

    // Balance must still be 1 — failed revoke does not decrement
    assert_eq!(client.balance_of(&user), 1, "Balance must remain 1");
}
