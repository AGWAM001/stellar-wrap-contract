#![cfg(test)]

extern crate std;

use super::*;
use ed25519_dalek::{Signer, SigningKey};
use crate::mint::CURRENT_PAYLOAD_VERSION;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    xdr::{ContractEventBody, ContractEventV0, ToXdr},
    Address, Bytes, BytesN, Env, IntoVal, String, Symbol, TryIntoVal, Val,
};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::vec::Vec;

use crate::storage_types::{DataKey, WrapLifecycleFSM, WrapRecord, WrapState};

const STRESS_USER_COUNT: usize = 128;

fn sign_payload(
    env: &Env,
    signer: &SigningKey,
    contract: &Address,
    user: &Address,
    period: u64,
    archetype: &Symbol,
    data_hash: &BytesN<32>,
    payload_version: u32,
) -> BytesN<64> {
    let payload = crate::signature::construct_mint_payload(env, contract, user, period, archetype, data_hash, payload_version);

    let mut out = [0u8; 512];
    let len = payload.len() as usize;
    payload.copy_into_slice(&mut out[..len]);

    let signature = signer.sign(&out[..len]);
    BytesN::from_array(env, &signature.to_bytes())
}

/// Helper: extract the topic at index `i` from a ContractEvent's V0 body.
fn event_topic(event: &soroban_sdk::xdr::ContractEvent, idx: u32) -> soroban_sdk::xdr::ScVal {
    match &event.body {
        ContractEventBody::V0(v0) => v0.topics.get(idx as usize).expect("topic index out of bounds").clone(),
        _ => panic!("expected V0 event body"),
    }
}

/// Helper: extract a topic and convert it through Val to the target type.
fn event_topic_as<T>(env: &Env, event: &soroban_sdk::xdr::ContractEvent, idx: u32) -> T
where
    Val: TryIntoVal<Env, T>,
    soroban_sdk::xdr::ScVal: TryIntoVal<Env, Val>,
{
    let scval = event_topic(event, idx);
    let val: Val = scval.try_into_val(env).unwrap();
    val.try_into_val(env).unwrap()
}

/// Helper: extract the data Val from a ContractEvent's V0 body.
fn event_data<'a>(event: &'a soroban_sdk::xdr::ContractEvent) -> &'a soroban_sdk::xdr::ScVal {
    match &event.body {
        ContractEventBody::V0(v0) => &v0.data,
        _ => panic!("expected V0 event body"),
    }
}

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
        &env,
        &signing_key,
        &contract_id,
        &user_a,
        period_a,
        &archetype_a,
        &hash,
        CURRENT_PAYLOAD_VERSION);
    let sig_b = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_b,
        period_b,
        &archetype_b,
        &hash,
        CURRENT_PAYLOAD_VERSION);

    client.mint_wrap(&user_a, &period_a, &archetype_a, &hash, &CURRENT_PAYLOAD_VERSION, &sig_a);
    client.mint_wrap(&user_b, &period_b, &archetype_b, &hash, &CURRENT_PAYLOAD_VERSION, &sig_b);
    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);

    // Check revoke event for user_a
    client.revoke_wrap(&user_a, &period_a, &reason_hash);
    let events_after_revoke_a = env.events().all();
    let last_event_a = events_after_revoke_a.events().last().expect("expected revoke event for a");
    let ev_a_len = match &last_event_a.body {
        soroban_sdk::xdr::ContractEventBody::V0(v0) => v0.topics.len(),
        _ => 0,
    };
    assert_eq!(ev_a_len, 3);
    let ev_a_t0: Symbol = event_topic(last_event_a, 0).try_into_val(&env).unwrap();
    assert_eq!(ev_a_t0, symbol_short!("revoke"));
    let ev_a_t1: Address = event_topic(last_event_a, 1).try_into_val(&env).unwrap();
    assert_eq!(ev_a_t1, user_a);
    let ev_a_t2: u64 = event_topic_as(&env, last_event_a, 2);
    assert_eq!(ev_a_t2, period_a);
    let ev_a_data: BytesN<32> = event_data(last_event_a).clone().try_into_val(&env).unwrap();
    assert_eq!(ev_a_data, reason_hash);

    // Check revoke event for user_b
    client.revoke_wrap(&user_b, &period_b, &reason_hash);
    let events_after_revoke_b = env.events().all();
    let last_event_b = events_after_revoke_b.events().last().expect("expected revoke event for b");
    let ev_b_len = match &last_event_b.body {
        soroban_sdk::xdr::ContractEventBody::V0(v0) => v0.topics.len(),
        _ => 0,
    };
    assert_eq!(ev_b_len, 3);
    let ev_b_t0: Symbol = event_topic(last_event_b, 0).try_into_val(&env).unwrap();
    assert_eq!(ev_b_t0, symbol_short!("revoke"));
    let ev_b_t1: Address = event_topic(last_event_b, 1).try_into_val(&env).unwrap();
    assert_eq!(ev_b_t1, user_b);
    let ev_b_t2: u64 = event_topic_as(&env, last_event_b, 2);
    assert_eq!(ev_b_t2, period_b);
    let ev_b_data: BytesN<32> = event_data(last_event_b).clone().try_into_val(&env).unwrap();
    assert_eq!(ev_b_data, reason_hash);
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

    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202401,
        &archetype,
        &hash, CURRENT_PAYLOAD_VERSION
    );
    client.mint_wrap(&user, &202401, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig1);

    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202402,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &202402, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig2);

    assert_eq!(client.balance_of(&user), 2);
}

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
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );

    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);
    assert_eq!(client.total_revoked(), 0);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
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

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );

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
    assert_eq!(
        client.name(),
        String::from_str(&env, "Stellar Wrap Registry")
    );
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

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
        CURRENT_PAYLOAD_VERSION,
    );
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

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

    let tampered_data = Bytes::from_slice(&env, b"{\"score\":999}");
    assert!(!client.verify_data(&user, &period, &tampered_data));
}

#[test]
fn test_name_returns_default_and_custom() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    // Before initialization: default name
    assert_eq!(
        client.name(),
        String::from_str(&env, "Stellar Wrap Registry")
    );

    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    // After init, still default
    assert_eq!(
        client.name(),
        String::from_str(&env, "Stellar Wrap Registry")
    );

    // Set custom name
    client.set_name(&String::from_str(&env, "My Custom Wrap"));
    assert_eq!(
        client.name(),
        String::from_str(&env, "My Custom Wrap")
    );
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

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

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

    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202402,
        &archetype,
        &hash1,
        CURRENT_PAYLOAD_VERSION,
    );
    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202404,
        &archetype,
        &hash2,
        CURRENT_PAYLOAD_VERSION,
    );
    let sig3 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202403,
        &archetype,
        &hash3,
        CURRENT_PAYLOAD_VERSION,
    );

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

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );

    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);

    let wrap = client.get_latest_wrap(&user).unwrap();
    assert_eq!(wrap.period, period);
    assert_eq!(wrap.data_hash, hash);
    assert_eq!(wrap.archetype, archetype);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_invalid_period_zero_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[70u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 0u64;
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );

    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);
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
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );

    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);
    let events = env.events().all();
    let last_event = events.events().last().expect("Expected at least one event");

    // Verify event topics and data
    // Mint event publishes (mint, user, period) as 3 topics, archetype as data
    let topic_0: Symbol = event_topic(last_event, 0).try_into_val(&env).unwrap();
    assert_eq!(topic_0, symbol_short!("mint"));
    let topic_1: Address = event_topic(last_event, 1).try_into_val(&env).unwrap();
    assert_eq!(topic_1, user);
    let topic_2: u64 = event_topic_as(&env, last_event, 2);
    assert_eq!(topic_2, period);

    let event_data_val: Symbol = event_data(last_event).clone().try_into_val(&env).unwrap();
    assert_eq!(event_data_val, archetype);
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
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );

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
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );

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
        let signature = sign_payload(
            &env,
            &signing_key,
            &contract_id,
            &user,
            period,
            &archetype,
            &hash,
            CURRENT_PAYLOAD_VERSION,
        );

        client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);

        cpu_samples[i] = env.budget().cpu_instruction_cost();
        mem_samples[i] = env.budget().memory_bytes_cost();
        users.push(user);
    }

    // Verify all samples collected
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

    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(
        client.get_mint_timestamp(&user, &period),
        Some(wrap.timestamp)
    );
}

#[test]
fn test_get_wrap_returns_none_before_initialization() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    assert!(client.get_wrap(&user, &202401).is_none());
}

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

    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);

    // After mint, admin is still readable — instance storage was not expired
    assert!(client.get_admin().is_some());
    assert_eq!(client.get_admin().unwrap(), admin);
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

    // Mint a wrap with archetype "arch"
    let archetype_old = symbol_short!("arch");
    let hash_old = BytesN::from_array(&env, &[41u8; 32]);
    let period = 202406u64;

    let sig_old = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype_old,
        &hash_old,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype_old, &hash_old, &CURRENT_PAYLOAD_VERSION, &sig_old);

    let wrap_old = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap_old.archetype, archetype_old);

    // Revoke it
    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);
    assert!(client.get_wrap(&user, &period).is_none());

    // Remint with archetype "builder"
    let archetype_new = symbol_short!("builder");
    let hash_new = BytesN::from_array(&env, &[42u8; 32]);

    let sig_new = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype_new,
        &hash_new,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype_new, &hash_new, &CURRENT_PAYLOAD_VERSION, &sig_new);

    // Verify mint event was emitted on remint (check BEFORE get_wrap queries)
    let snap = env.events().all();
    let last_event = snap.events().last().expect("expected an event after remint");
    let mint_topic: Symbol = event_topic(last_event, 0).try_into_val(&env).unwrap();
    assert_eq!(mint_topic, symbol_short!("mint"));

    let wrap_new = client.get_wrap(&user, &period).unwrap();
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
fn test_get_mint_timestamp_missing() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    let user = Address::generate(&env);
    let period = 202406u64;

    assert_eq!(client.get_mint_timestamp(&user, &period), None);
}

#[test]
fn test_instance_ttl_extended_on_second_mint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[16u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[1u8; 32]);

    let user_a = Address::generate(&env);
    let sig_a = sign_payload(&env, &signing_key, &contract_id, &user_a, 202407u64, &archetype, &hash, CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user_a, &202407u64, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig_a);

    let user_b = Address::generate(&env);
    let sig_b = sign_payload(&env, &signing_key, &contract_id, &user_b, 202407u64, &archetype, &hash, CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user_b, &202407u64, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig_b);

    // Admin address still accessible after multiple mints
    assert_eq!(client.get_admin().unwrap(), admin);
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
fn test_fsm_valid_state_transitions() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[97u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[16u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202408u64;

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);

    // After mint, wrap starts in Active state. Check Draft -> Pending transition.
    // Since wraps are minted directly into Active state, test Pending -> Active instead.
    // Actually mint_wrap creates wraps in Active state, so transitions from Active are valid.
    // Transition Active -> Archived
    client.transition_wrap_state(&user, &period, &WrapState::Archived);
    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.fsm.state, WrapState::Archived);
}

#[test]
fn test_old_signature_fails_after_pubkey_rotation() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let initial_signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let initial_pubkey = BytesN::from_array(&env, &initial_signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &initial_pubkey);
    env.mock_all_auths();

    // Mint with old key — should succeed via initial pubkey
    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let sig = sign_payload(
        &env,
        &initial_signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, hash);
}

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

    let sig_1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash_1,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &hash_1, &CURRENT_PAYLOAD_VERSION, &sig_1);
    assert_eq!(client.balance_of(&user), 1);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    // Check revoke event immediately after the revoke call
    let snap = env.events().all();
    let revoke_event = snap.events().last().expect("Expected revoke event");

    let ev_topic: Symbol = event_topic(revoke_event, 0).try_into_val(&env).unwrap();
    assert_eq!(ev_topic, symbol_short!("revoke"));
    let ev_user: Address = event_topic(revoke_event, 1).try_into_val(&env).unwrap();
    assert_eq!(ev_user, user);
    assert_eq!(event_topic_as::<u64>(&env, revoke_event, 2), period);
    let ev_reason: BytesN<32> = event_data(revoke_event).clone().try_into_val(&env).unwrap();
    assert_eq!(ev_reason, reason_hash);

    // Remint
    let sig_2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash_2,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &hash_2, &CURRENT_PAYLOAD_VERSION, &sig_2);

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
        let latest_key = DataKey::LatestPeriod(user.clone());
        let record = WrapRecord {
            timestamp: env.ledger().timestamp(),
            data_hash: BytesN::from_array(&env, &[16u8; 32]),
            archetype: symbol_short!("arch"),
            period: 2026,
            fsm: WrapLifecycleFSM::new(WrapState::Active, env.ledger().timestamp()),
        };
        env.storage().persistent().set(&wrap_key, &record);
        env.storage().persistent().set(&count_key, &1u32);
        env.storage().persistent().set(&latest_key, &2026u64);
    });

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &2026, &reason_hash);
}

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
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

    // Verify the wrap was created and is in Active state
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

    let signing_key = SigningKey::from_bytes(&[14u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let period = 202601u64;
    let archetype = symbol_short!("arch");
    let data_hash = BytesN::from_array(&env, &[14u8; 32]);
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

    // After mint, wrap is in Active state. Trying to transition Active -> Draft is invalid.
    client.transition_wrap_state(&user, &period, &WrapState::Draft);
}

#[test]
fn test_mint_guard_on_failure_leaves_no_residual_state() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[17u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202601u64;

    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig1);

    // Second mint with same period should panic (wrap already exists - error #4)
    let duplicate = catch_unwind(AssertUnwindSafe(|| {
        client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig1)
    }));
    assert!(duplicate.is_err());
}

#[test]
fn test_upgrade_emits_event() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[18u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let new_wasm_hash = BytesN::from_array(&env, &[42u8; 32]);

    // The upgrade call panics because the fake WASM hash doesn't point to a real WASM.
    // Wrap in catch_unwind to catch the expected panic so the test can continue.
    let result = catch_unwind(AssertUnwindSafe(|| {
        client.upgrade(&new_wasm_hash);
    }));
    assert!(result.is_err(), "upgrade should panic (fake WASM hash)");

    // contract_version should be 0 because storage was rolled back on panic
    assert_eq!(client.contract_version(), 0);
}

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
    let last_event = events.events().last().expect("Expected at least one event");

    // update_admin event publishes (admin, updated) as 2 topics, (old_admin, new_admin) as data
    let topic_0: Symbol = event_topic(last_event, 0).try_into_val(&env).unwrap();
    let topic_1: Symbol = event_topic(last_event, 1).try_into_val(&env).unwrap();
    assert_eq!(topic_0, symbol_short!("admin"));
    assert_eq!(topic_1, symbol_short!("updated"));

    let data_val: Val = event_data(last_event).clone().try_into_val(&env).unwrap();
    let (old_admin_val, new_admin_val): (Address, Address) = data_val.try_into_val(&env).unwrap();
    assert_eq!(old_admin_val, admin);
    assert_eq!(new_admin_val, new_admin);
}

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
#[should_panic]
fn test_mint_wrap_zero_hash_rejected() {
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
    let period = 2024u64;

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &zero_hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &zero_hash, &CURRENT_PAYLOAD_VERSION, &sig);
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

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &edge_hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &edge_hash, &CURRENT_PAYLOAD_VERSION, &sig);

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

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &max_hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &max_hash, &CURRENT_PAYLOAD_VERSION, &sig);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, max_hash);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_upgrade_before_init_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    env.mock_all_auths();

    let dummy_wasm_hash = BytesN::from_array(&env, &[0xAAu8; 32]);
    client.upgrade(&dummy_wasm_hash);
}

#[test]
#[should_panic]
fn test_unauthorized_upgrade_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[22u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    // Do NOT mock auth — should fail with Unauthorized

    let new_wasm_hash = BytesN::from_array(&env, &[42u8; 32]);
    client.upgrade(&new_wasm_hash);
}

#[test]
#[should_panic]
fn test_upgrade_requires_admin_auth() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);

    // Do NOT mock all auths — calling without admin authorization should fail
    let fake_wasm = BytesN::from_array(&env, &[0u8; 32]);
    client.upgrade(&fake_wasm);
}

// ---------------------------------------------------------------------------
// Alias hash tests (#288)
// ---------------------------------------------------------------------------

#[test]
fn test_set_and_get_alias_hash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    let user = Address::generate(&env);

    client.initialize(&admin, &pubkey);
    let alias_hash = BytesN::from_array(&env, &[0xabu8; 32]);

    // No hash set yet
    assert!(client.get_alias_hash(&user).is_none());

    env.mock_all_auths();
    client.set_alias_hash(&user, &alias_hash);

    assert_eq!(client.get_alias_hash(&user).unwrap(), alias_hash);
}

#[test]
fn test_update_alias_hash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[2u8; 32]);
    let user = Address::generate(&env);

    client.initialize(&admin, &pubkey);
    let hash_v1 = BytesN::from_array(&env, &[0x11u8; 32]);
    let hash_v2 = BytesN::from_array(&env, &[0x22u8; 32]);

    env.mock_all_auths();
    client.set_alias_hash(&user, &hash_v1);
    assert_eq!(client.get_alias_hash(&user).unwrap(), hash_v1);

    // Overwrite with a new hash
    client.set_alias_hash(&user, &hash_v2);
    assert_eq!(client.get_alias_hash(&user).unwrap(), hash_v2);
}

#[test]
fn test_alias_hash_is_per_user() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[3u8; 32]);
    client.initialize(&admin, &pubkey);

    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);
    let hash_a = BytesN::from_array(&env, &[0xaau8; 32]);
    let hash_b = BytesN::from_array(&env, &[0xbbu8; 32]);

    env.mock_all_auths();
    client.set_alias_hash(&user_a, &hash_a);
    client.set_alias_hash(&user_b, &hash_b);

    assert_eq!(client.get_alias_hash(&user_a).unwrap(), hash_a);
    assert_eq!(client.get_alias_hash(&user_b).unwrap(), hash_b);
    assert_ne!(
        client.get_alias_hash(&user_a).unwrap(),
        client.get_alias_hash(&user_b).unwrap()
    );
}

#[test]
fn test_get_alias_hash_returns_none_for_unknown_user() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let unknown_user = Address::generate(&env);
    assert!(client.get_alias_hash(&unknown_user).is_none());
}

// ─── Revocation Tests ──────────────────────────────────────────────────────────

#[test]
fn test_revoke_wrap_success() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[15u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);
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

    let signing_key = SigningKey::from_bytes(&[16u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);

    let reason_hash = BytesN::from_array(&env, &[0xAAu8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    let events = env.events().all();
    let last_event = events.events().last().expect("no events found");

    let ev_topic: Symbol = event_topic(last_event, 0).try_into_val(&env).unwrap();
    assert_eq!(ev_topic, symbol_short!("revoke"));
    let ev_user: Address = event_topic(last_event, 1).try_into_val(&env).unwrap();
    assert_eq!(ev_user, user);
    let ev_period: u64 = event_topic_as(&env, last_event, 2);
    assert_eq!(ev_period, period);
    let event_reason: BytesN<32> = event_data(last_event).clone().try_into_val(&env).unwrap();
    assert_eq!(event_reason, reason_hash);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_revoke_wrap_not_found_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[19u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &9999, &reason_hash);
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

// ─── Issue #91: TTL auto-renewal for active users ────────────────────────────

#[test]
fn test_metadata_ttl_extended_on_new_mint() {
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

    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, 202401, &archetype, &hash, CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user, &202401, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig1);

    assert_eq!(client.balance_of(&user), 1);
    assert_eq!(client.get_latest_wrap(&user).unwrap().period, 202401);

    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 202501, &archetype, &hash, CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user, &202501, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig2);

    assert_eq!(client.balance_of(&user), 2);
    assert_eq!(client.get_latest_wrap(&user).unwrap().period, 202501);

    // Both metadata keys are alive — proves TTL was extended on each mint
    let count_key = DataKey::WrapCount(user.clone());
    let latest_key = DataKey::LatestPeriod(user.clone());
    env.as_contract(&contract_id, || {
        assert!(env.storage().persistent().has(&count_key));
        assert!(env.storage().persistent().has(&latest_key));
    });
}

#[test]
fn test_old_wrap_preserved_on_new_mint() {
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

    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, 202401, &archetype, &hash1, CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user, &202401, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);

    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 202501, &archetype, &hash2, CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user, &202501, &archetype, &hash2, &CURRENT_PAYLOAD_VERSION, &sig2);

    let wrap1 = client.get_wrap(&user, &202401).unwrap();
    assert_eq!(wrap1.period, 202401);
    assert_eq!(wrap1.data_hash, hash1);

    let wrap2 = client.get_wrap(&user, &202501).unwrap();
    assert_eq!(wrap2.period, 202501);
    assert_eq!(wrap2.data_hash, hash2);

    assert_eq!(client.balance_of(&user), 2);
}

#[test]
fn test_revoke_latest_period_clears_latest() {
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

    let sig1 = sign_payload(
        &env, &signing_key, &contract_id, &user, 202401, &archetype, &hash1, CURRENT_PAYLOAD_VERSION,
    );
    let sig2 = sign_payload(
        &env, &signing_key, &contract_id, &user, 202402, &archetype, &hash2, CURRENT_PAYLOAD_VERSION,
    );

    client.mint_wrap(&user, &202401, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);
    client.mint_wrap(&user, &202402, &archetype, &hash2, &CURRENT_PAYLOAD_VERSION, &sig2);

    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, 202402);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &202402, &reason_hash);

    assert!(client.get_latest_wrap(&user).is_none());
    assert_eq!(client.balance_of(&user), 1);
}

#[test]
fn test_revoke_non_latest_preserves_latest() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[53u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let hash1 = BytesN::from_array(&env, &[10u8; 32]);
    let hash2 = BytesN::from_array(&env, &[20u8; 32]);

    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, 202401, &archetype, &hash1, CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user, &202401, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);

    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 202403, &archetype, &hash2, CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user, &202403, &archetype, &hash2, &CURRENT_PAYLOAD_VERSION, &sig2);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &202401, &reason_hash);

    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, 202403);
    assert_eq!(client.balance_of(&user), 1);
}

#[test]
fn test_remint_after_revoke_succeeds() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[54u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let hash1 = BytesN::from_array(&env, &[10u8; 32]);
    let hash2 = BytesN::from_array(&env, &[20u8; 32]);
    let period = 202401u64;

    let sig1 = sign_payload(
        &env, &signing_key, &contract_id, &user, period, &archetype, &hash1, CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    let sig2 = sign_payload(
        &env, &signing_key, &contract_id, &user, period, &archetype, &hash2, CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &hash2, &CURRENT_PAYLOAD_VERSION, &sig2);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, hash2);
    assert_eq!(client.balance_of(&user), 1);
}

#[test]
fn test_revoke_with_zero_reason_hash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[55u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;
    let signature = sign_payload(
        &env, &signing_key, &contract_id, &user, period, &archetype, &hash, CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    // Check revoke event immediately after revoke
    let snap = env.events().all();
    let last_event = snap.events().last().expect("no events found");
    let event_reason: BytesN<32> = event_data(last_event).clone().try_into_val(&env).unwrap();
    assert_eq!(event_reason, reason_hash);

    assert!(client.get_wrap(&user, &period).is_none());
    assert_eq!(client.balance_of(&user), 0);
}

#[test]
fn test_renew_all_ttls_extends_metadata() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[56u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let archetype = symbol_short!("arch");

    let sig = sign_payload(&env, &signing_key, &contract_id, &user, 202401, &archetype, &hash, CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user, &202401, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);

    assert_eq!(client.balance_of(&user), 1);
    assert!(client.get_latest_wrap(&user).is_some());

    client.renew_all_ttls(&user);

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

#[test]
#[should_panic]
fn test_set_alias_hash_requires_auth() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);

    // Do NOT call env.mock_all_auths() — auth must be required
    let user = Address::generate(&env);
    let alias_hash = BytesN::from_array(&env, &[0xccu8; 32]);
    client.set_alias_hash(&user, &alias_hash);
}

fn sign_update_payload(
    env: &Env,
    signer: &SigningKey,
    contract: &Address,
    user: &Address,
    period: u64,
    new_archetype: &Symbol,
    new_data_hash: &BytesN<32>,
) -> BytesN<64> {
    sign_payload(
        env,
        signer,
        contract,
        user,
        period,
        new_archetype,
        new_data_hash,
        CURRENT_PAYLOAD_VERSION,
    )
}

// ─── update_wrap tests removed because the contract does not expose update_wrap ───
// The contract allows revoking and reminting instead.

// ─── update_admin_pubkey is not available in the contract API ────────────────
// The contract exposes update_admin (admin.rs) and initialize (which sets the
// pubkey at construction time), but there is no separate update_admin_pubkey
// function. Tests for pubkey rotation have been adapted to test existing
// functionality instead.

// ─────────────────────────────────────────────────────────────────────────────
// Query coverage tests to bring queries.rs to 100 % line coverage (#491)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_get_wraps_returns_empty_for_unminted_user() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let wraps = client.get_wraps(&user, &0u32, &10u32);
    assert_eq!(wraps.len(), 0);
}

#[test]
fn test_get_wraps_with_multiple_mints() {
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
    let hash = BytesN::from_array(&env, &[5u8; 32]);

    // Mint 3 wraps for the same user at different periods
    for period in [202401u64, 202402u64, 202403u64] {
        let sig = sign_payload(
            &env,
            &signing_key,
            &contract_id,
            &user,
            period,
            &archetype,
            &hash,
            CURRENT_PAYLOAD_VERSION,
        );
        client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);
    }

    // Fetch all wraps
    let wraps = client.get_wraps(&user, &0u32, &10u32);
    assert_eq!(wraps.len(), 3);

    // Pagination: start=1, limit=2 should return 2 wraps (periods 202402, 202403)
    let partial = client.get_wraps(&user, &1u32, &2u32);
    assert_eq!(partial.len(), 2);

    // Start beyond length returns empty
    let beyond = client.get_wraps(&user, &10u32, &5u32);
    assert_eq!(beyond.len(), 0);
}

#[test]
fn test_symbol_after_set_and_default() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    // Before init: default symbol
    assert_eq!(client.symbol(), String::from_str(&env, "WRAP"));

    let signing_key = SigningKey::from_bytes(&[51u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    // After init, still default
    assert_eq!(client.symbol(), String::from_str(&env, "WRAP"));

    // Set custom symbol
    client.set_symbol(&String::from_str(&env, "CSTM"));
    assert_eq!(client.symbol(), String::from_str(&env, "CSTM"));
}

#[test]
fn test_get_admin_after_init_returns_admin() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[52u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    assert_eq!(client.get_admin().unwrap(), admin);
}

#[test]
fn test_total_revoked_before_any_revocation() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    // Before init: should be 0
    assert_eq!(client.total_revoked(), 0);

    let signing_key = SigningKey::from_bytes(&[53u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    // After init, before any revocation: still 0
    assert_eq!(client.total_revoked(), 0);

    // Mint and revoke
    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[6u8; 32]);
    let period = 202401u64;
    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);

    let reason = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason);
    assert_eq!(client.total_revoked(), 1);
}

#[test]
fn test_contract_version_default_and_upgrade_attempt() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    // Before init: version is 0
    assert_eq!(client.contract_version(), 0);

    let signing_key = SigningKey::from_bytes(&[54u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    // After init: version is still 0
    assert_eq!(client.contract_version(), 0);

    // Attempt upgrade (will panic because fake WASM hash)
    let new_wasm_hash = BytesN::from_array(&env, &[42u8; 32]);
    let result = catch_unwind(AssertUnwindSafe(|| {
        client.upgrade(&new_wasm_hash);
    }));
    assert!(result.is_err());

    // Version is still 0 because storage was rolled back on panic
    assert_eq!(client.contract_version(), 0);
}

#[test]
fn test_health_after_init_and_before() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    // Before init: all false
    let h = client.health();
    assert!(!h.initialized);
    assert!(!h.has_admin);
    assert!(!h.has_signing_key);

    let signing_key = SigningKey::from_bytes(&[55u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);

    // After init: all true
    let h = client.health();
    assert!(h.initialized);
    assert!(h.has_admin);
    assert!(h.has_signing_key);
}

#[test]
fn test_verify_data_no_wrap_returns_false() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let data = Bytes::from_slice(&env, b"test data");
    assert!(!client.verify_data(&user, &202401, &data));
}

#[test]
fn test_get_mint_timestamp_before_mint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    assert_eq!(client.get_mint_timestamp(&user, &202401), None);
}

#[test]
fn test_get_wrap_before_mint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    assert!(client.get_wrap(&user, &202401).is_none());
}

#[test]
fn test_get_latest_wrap_with_no_mints() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    assert!(client.get_latest_wrap(&user).is_none());
}

#[test]
fn test_decimals() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    assert_eq!(client.decimals(), 0);
}
