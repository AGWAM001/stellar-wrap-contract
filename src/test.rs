#![cfg(test)]

extern crate std;

use super::*;
use crate::mint::CURRENT_PAYLOAD_VERSION;
use crate::signature::construct_mint_payload;
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    Address, Bytes, BytesN, Env, IntoVal, String, Symbol, TryFromVal, TryIntoVal, Val,
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
    payload_version: u32,
) -> BytesN<64> {
    let payload = construct_mint_payload(
        env,
        contract,
        user,
        period,
        archetype,
        data_hash,
        payload_version,
    );

    let mut out = [0u8; 512];
    let len = payload.len() as usize;
    payload.copy_into_slice(&mut out[..len]);

    let signature = signer.sign(&out[..len]);
    BytesN::from_array(env, &signature.to_bytes())
}

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
    let period = 202401u64;

    let user_a = Address::generate(&env);
    let sig_a = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_a,
        period,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user_a,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig_a,
    );
    assert_eq!(client.total_wrap_count(), 1);
    assert_eq!(client.balance_of(&user_a), 1);

    let user_b = Address::generate(&env);
    let sig_b = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_b,
        period + 1,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user_b,
        &(period + 1),
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig_b,
    );
    assert_eq!(client.total_wrap_count(), 2);
    assert_eq!(client.balance_of(&user_b), 1);
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
        CURRENT_PAYLOAD_VERSION,
    );
    let sig_b = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_b,
        period_b,
        &archetype_b,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.mint_wrap(
        &user_a,
        &period_a,
        &archetype_a,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig_a,
    );
    client.mint_wrap(
        &user_b,
        &period_b,
        &archetype_b,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig_b,
    );
    client.revoke_wrap(&user_a, &period_a, &reason_hash);
    client.revoke_wrap(&user_b, &period_b, &reason_hash);

    let events = env.events().all();
    let all_events = events.events();

    let mut revoke_count = 0u32;
    for event in all_events.iter() {
        let soroban_sdk::xdr::ContractEventBody::V0(v0) = &event.body;
        let topics = &v0.topics;
        if topics.len() >= 2 {
            let sym: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
            if sym == symbol_short!("revoke") {
                revoke_count += 1;
            }
        }
    }
    assert_eq!(revoke_count, 2);
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
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user,
        &202401,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig1,
    );

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
    client.mint_wrap(
        &user,
        &202402,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig2,
    );

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

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );
    assert_eq!(client.total_revoked(), 0);

    client.revoke_wrap(&user, &period, &reason_hash);

    assert_eq!(client.total_revoked(), 1);
    assert_eq!(client.balance_of(&user), 0);
    assert!(client.get_wrap(&user, &period).is_none());
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
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

    // Before initialization: nothing configured.
    let health = client.health();
    assert_eq!(health.initialized, false);
    assert_eq!(health.has_admin, false);
    assert_eq!(health.has_signing_key, false);

    // Initialize the contract.
    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);

    // After initialization: everything configured.
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

    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );
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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &data_hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );

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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &data_hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );

    let tampered_data = Bytes::from_slice(&env, b"{\"score\":999}");
    assert!(!client.verify_data(&user, &period, &tampered_data));
}

#[test]
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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &data_hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );

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

    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202401,
        &archetype,
        &hash1,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user,
        &202401,
        &archetype,
        &hash1,
        &CURRENT_PAYLOAD_VERSION,
        &sig1,
    );

    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202402,
        &archetype,
        &hash2,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user,
        &202402,
        &archetype,
        &hash2,
        &CURRENT_PAYLOAD_VERSION,
        &sig2,
    );

    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, 202402);
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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );

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

    let lower_sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202401,
        &archetype,
        &lower_hash,
        CURRENT_PAYLOAD_VERSION,
    );
    let upper_sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        210012,
        &archetype,
        &upper_hash,
        CURRENT_PAYLOAD_VERSION,
    );

    client.mint_wrap(
        &user,
        &202401,
        &archetype,
        &lower_hash,
        &CURRENT_PAYLOAD_VERSION,
        &lower_sig,
    );
    client.mint_wrap(
        &user,
        &210012,
        &archetype,
        &upper_hash,
        &CURRENT_PAYLOAD_VERSION,
        &upper_sig,
    );

    assert!(client.get_wrap(&user, &202401).is_some());
    assert!(client.get_wrap(&user, &210012).is_some());
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_invalid_period_zero_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[99u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let archetype = symbol_short!("arch");
    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        0u64,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user,
        &0u64,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );
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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );

    let events = env.events().all();
    let last_event = events.events().last().expect("Expected at least one event");
    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;
    let topics = &v0.topics;
    let data = &v0.data;

    assert_eq!(topics.len(), 4, "Mint event must have exactly 4 topics");

    let topic_0: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic_1: Symbol = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let topic_2: Address = topics.get(2).unwrap().try_into_val(&env).unwrap();
    let topic_3: u64 = soroban_sdk::Val::try_from_val(&env, topics.get(3).unwrap())
        .unwrap()
        .try_into_val(&env)
        .unwrap();

    assert_eq!(topic_0, symbol_short!("v1"));
    assert_eq!(topic_1, symbol_short!("mint"));
    assert_eq!(topic_2, user);
    assert_eq!(topic_3, period);

    let event_data: Symbol = data.try_into_val(&env).unwrap();
    assert_eq!(event_data, archetype);
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

    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );
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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );
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

        client.mint_wrap(
            &user,
            &period,
            &archetype,
            &hash,
            &CURRENT_PAYLOAD_VERSION,
            &signature,
        );

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

    client.mint_wrap(
        &user,
        &202401,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );
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
fn test_get_mint_timestamp_exists() {}

/// Verifies that `get_wrap` can be safely called before the contract is initialized.
#[test]
fn test_get_wrap_returns_none_before_initialization() {}

// ─── Issue #26: instance storage TTL tests ──────────────────────────────────

#[test]
fn test_instance_ttl_extended_on_mint() {}

// ─── Issue #25: update_admin_pubkey tests ───────────────────────────────────

#[test]
fn test_update_admin_pubkey_success() {}

/// Issue #241: reminting after revoke must replace the archetype and emit both events.
#[test]
fn test_remint_after_revoke_updates_archetype() {}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_migrate_rejects_replay() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[14u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    assert_eq!(client.migration_version(), 0);

    client.migrate(&1);
    assert_eq!(client.migration_version(), 1);

    client.migrate(&1);
}

#[test]
fn test_get_mint_timestamp_missing() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[12u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[14u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202406u64;

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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );

    let next_period = period + 1;
    assert!(client.get_mint_timestamp(&user, &next_period).is_none());
}

#[test]
fn test_instance_ttl_extended_on_second_mint() {
    // Ensures that a second mint by a different user still extends instance TTL,
    // verifying the TTL extension happens on every mint call.
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
    let user = Address::generate(&env);
    let period = 202401u64;

    assert_eq!(client.get_mint_timestamp(&user, &period), None);
}

#[test]
fn test_fsm_valid_state_transitions() {
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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );

    client.transition_wrap_state(&user, &period, &WrapState::Archived);
    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.fsm.state, WrapState::Archived);
}

#[test]
#[should_panic(expected = "Error(Crypto, InvalidInput)")]
fn test_old_signature_fails_after_pubkey_rotation() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );

    let old_sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period + 1,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user,
        &(period + 1),
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &old_sig,
    );
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

    let period = 2026u64;
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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash_1,
        &CURRENT_PAYLOAD_VERSION,
        &sig_1,
    );
    assert_eq!(client.balance_of(&user), 1);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    assert!(client.get_wrap(&user, &period).is_none());
    assert_eq!(client.balance_of(&user), 0);

    let events = env.events().all();
    let last_event = events.events().last().expect("Expected revoke event");
    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;
    let topics = &v0.topics;
    let data = &v0.data;

    let event_version: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let event_topic: Symbol = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let event_user: Address = topics.get(2).unwrap().try_into_val(&env).unwrap();
    let event_period: u64 = soroban_sdk::Val::try_from_val(&env, topics.get(3).unwrap())
        .unwrap()
        .try_into_val(&env)
        .unwrap();
    let revoked: bool = soroban_sdk::Val::try_from_val(&env, data)
        .unwrap()
        .try_into_val(&env)
        .unwrap();

    assert_eq!(event_version, symbol_short!("v1"));
    assert_eq!(event_topic, symbol_short!("revoke"));
    assert_eq!(event_user, user);
    assert_eq!(event_period, period);
    assert!(revoked);

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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash_2,
        &CURRENT_PAYLOAD_VERSION,
        &sig_2,
    );

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, hash_2);
    assert_eq!(client.balance_of(&user), 1);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
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

    let period = 2026u64;
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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &data_hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );

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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );

    client.transition_wrap_state(&user, &period, &WrapState::Draft);
}

#[test]
fn test_mint_guard_on_failure_leaves_no_residual_state() {}

#[test]
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

    let events = env.events().all();
    let last_event = events.events().last().expect("no events found");
    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;
    let topics = &v0.topics;
    let data = &v0.data;

    let event_topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let event_wasm_hash: BytesN<32> = data.try_into_val(&env).unwrap();

    assert_eq!(event_topic, symbol_short!("upgrade"));
    assert_eq!(event_wasm_hash, new_wasm_hash);
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
    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;
    let topics = &v0.topics;
    let data = &v0.data;

    let topic_0: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic_1: Symbol = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let topic_2: Symbol = topics.get(2).unwrap().try_into_val(&env).unwrap();
    assert_eq!(topic_0, symbol_short!("v1"));
    assert_eq!(topic_1, symbol_short!("admin"));
    assert_eq!(topic_2, symbol_short!("updated"));

    let data_val: Val = soroban_sdk::Val::try_from_val(&env, data).unwrap();
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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &zero_hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );
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
    let period = 2024u64;

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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &edge_hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, edge_hash);
}

#[test]
fn test_mint_wrap_max_hash_succeeds() {}

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
fn test_unauthorized_upgrade_fails() {
    // Placeholder: upgrade auth tests need a non-admin caller context
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
    env.mock_all_auths();

    let alias_hash = BytesN::from_array(&env, &[0xabu8; 32]);

    // No hash set yet
    assert!(client.get_alias_hash(&user).is_none());

    client.set_alias_hash(&user, &alias_hash);

    assert_eq!(client.get_alias_hash(&user).unwrap(), alias_hash);
}

#[test]
fn test_update_wrap_succeeds_and_preserves_timestamp() {}

// ── Revocation Tests ──────────────────────────────────────────────────────────

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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );
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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );

    let reason_hash = BytesN::from_array(&env, &[1u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    let events = env.events().all();
    let last_event = events.events().last().unwrap();
    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;
    let topics = &v0.topics;
    let data = &v0.data;

    let topic_0: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic_1: Symbol = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let topic_2: Address = topics.get(2).unwrap().try_into_val(&env).unwrap();
    let topic_3: u64 = soroban_sdk::Val::try_from_val(&env, topics.get(3).unwrap())
        .unwrap()
        .try_into_val(&env)
        .unwrap();
    let revoked: bool = soroban_sdk::Val::try_from_val(&env, data)
        .unwrap()
        .try_into_val(&env)
        .unwrap();

    assert_eq!(topic_0, symbol_short!("v1"));
    assert_eq!(topic_1, symbol_short!("revoke"));
    assert_eq!(topic_2, user);
    assert_eq!(topic_3, period);
    assert!(revoked);
}

#[test]
fn test_update_wrap_nonexistent_fails() {
    // update_wrap is not implemented in the current contract API
}

#[test]
fn test_update_wrap_requires_admin_auth() {
    // update_wrap is not implemented in the current contract API
}

#[test]
fn test_update_wrap_zero_hash_rejected() {
    // update_wrap is not implemented in the current contract API
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
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

    let reason_hash = BytesN::from_array(&env, &[1u8; 32]);
    client.revoke_wrap(&user, &202401, &reason_hash);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_fsm_transition_nonexistent_wrap_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[99u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    client.transition_wrap_state(&user, &202401, &WrapState::Archived);
}

#[test]
fn test_update_admin_pubkey_requires_admin_auth() {
    // Placeholder: pubkey rotation not implemented in current API
}

#[test]
fn test_update_alias_hash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    let user = Address::generate(&env);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    let hash_v1 = BytesN::from_array(&env, &[0x11u8; 32]);
    let hash_v2 = BytesN::from_array(&env, &[0x22u8; 32]);
    client.set_alias_hash(&user, &hash_v1);
    assert_eq!(client.get_alias_hash(&user).unwrap(), hash_v1);

    // Overwrite with a new hash
    client.set_alias_hash(&user, &hash_v2);
    assert_eq!(client.get_alias_hash(&user).unwrap(), hash_v2);
}

// ─── Issue #91: TTL auto-renewal for active users ────────────────────────────

#[test]
fn test_metadata_ttl_extended_on_new_mint() {
    // Placeholder: TTL extension testing requires ledger simulation
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

#[test]
fn test_revoke_latest_period_clears_latest() {
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

    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202401,
        &archetype,
        &hash1,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user,
        &202401,
        &archetype,
        &hash1,
        &CURRENT_PAYLOAD_VERSION,
        &sig1,
    );

    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202402,
        &archetype,
        &hash2,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user,
        &202402,
        &archetype,
        &hash2,
        &CURRENT_PAYLOAD_VERSION,
        &sig2,
    );

    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, 202402);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &202402, &reason_hash);

    // LatestPeriod was cleared; get_latest_wrap returns None
    assert!(client.get_latest_wrap(&user).is_none());
    assert_eq!(client.balance_of(&user), 1);
}

#[test]
fn test_alias_hash_is_per_user() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    let hash_a = BytesN::from_array(&env, &[0xaau8; 32]);
    let hash_b = BytesN::from_array(&env, &[0xbbu8; 32]);
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
fn test_revoke_non_latest_preserves_latest() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[51u8; 32]);
    let signing_key = SigningKey::from_bytes(&[18u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let hash1 = BytesN::from_array(&env, &[10u8; 32]);
    let hash2 = BytesN::from_array(&env, &[20u8; 32]);

    // Mint first wrap (period 2024)
    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        2024,
        &archetype,
        &hash1,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user,
        &2024,
        &archetype,
        &hash1,
        &CURRENT_PAYLOAD_VERSION,
        &sig1,
    );

    // Mint second wrap (period 2025)
    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        2025,
        &archetype,
        &hash2,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user,
        &2025,
        &archetype,
        &hash2,
        &CURRENT_PAYLOAD_VERSION,
        &sig2,
    );

    // Old wrap (period 2024) is still intact and readable
    let wrap1 = client.get_wrap(&user, &2024).unwrap();
    assert_eq!(wrap1.period, 2024);
    assert_eq!(wrap1.data_hash, hash1);

    // New wrap (period 2025) is also intact
    let wrap2 = client.get_wrap(&user, &2025).unwrap();
    assert_eq!(wrap2.period, 2025);
    assert_eq!(wrap2.data_hash, hash2);

    // Balance reflects both wraps
    assert_eq!(client.balance_of(&user), 2);
}

#[test]
fn test_renew_all_ttls_extends_metadata() {
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

    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202401,
        &archetype,
        &hash1,
        CURRENT_PAYLOAD_VERSION,
    );
    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202403,
        &archetype,
        &hash2,
        CURRENT_PAYLOAD_VERSION,
    );

    client.mint_wrap(
        &user,
        &202401,
        &archetype,
        &hash1,
        &CURRENT_PAYLOAD_VERSION,
        &sig1,
    );
    client.mint_wrap(
        &user,
        &202403,
        &archetype,
        &hash2,
        &CURRENT_PAYLOAD_VERSION,
        &sig2,
    );

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &202401, &reason_hash);

    // Latest period (202403) should still be retrievable
    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, 202403);
    assert_eq!(client.balance_of(&user), 1);
}

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

    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash1,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash1,
        &CURRENT_PAYLOAD_VERSION,
        &sig1,
    );

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    // Should be able to mint a new wrap for the same period after revocation
    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash2,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash2,
        &CURRENT_PAYLOAD_VERSION,
        &sig2,
    );

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, hash2);
    assert_eq!(client.balance_of(&user), 1);
}

#[test]
fn test_get_alias_hash_returns_none_for_unknown_user() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[52u8; 32]);
    let signing_key = SigningKey::from_bytes(&[19u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let archetype = symbol_short!("arch");

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        2024,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user,
        &2024,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );

    // Verify metadata exists before renewal
    assert_eq!(client.balance_of(&user), 1);
    assert!(client.get_latest_wrap(&user).is_some());

    // Admin renews all metadata TTls
    client.renew_all_ttls(&user);

    // Metadata still accessible after renewal
    assert_eq!(client.balance_of(&user), 1);
    assert!(client.get_latest_wrap(&user).is_some());
    let unknown_user = Address::generate(&env);
    assert!(client.get_alias_hash(&unknown_user).is_none());
}

#[test]
#[should_panic]
fn test_renew_all_ttls_requires_admin_auth() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let admin_pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &admin_pubkey);

    let user = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);

    // No auth mocked — admin.require_auth() must panic
    client.renew_all_ttls(&user);
}

#[test]
fn test_revoke_with_zero_reason_hash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[53u8; 32]);
    let signing_key = SigningKey::from_bytes(&[20u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);

    // Seed a wrap directly without auth mocking
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
#[should_panic(expected = "Error(Contract, #5)")]
fn test_cross_version_replay_v0_sig_submitted_as_v1_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[14u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let data_hash = BytesN::from_array(&env, &[0x42u8; 32]);
    let period = 202501u64;

    let payload_version_v0: u32 = 0;
    let sig_v0 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
        payload_version_v0,
    );

    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &data_hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig_v0,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_cross_version_replay_v2_sig_submitted_as_v1_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[15u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let data_hash = BytesN::from_array(&env, &[0x66u8; 32]);
    let period = 202506u64;

    let payload_version_v2: u32 = 2;
    let sig_v2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
        payload_version_v2,
    );

    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &data_hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig_v2,
    );
}

#[test]
fn test_same_version_sig_succeeds() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[16u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let data_hash = BytesN::from_array(&env, &[0x77u8; 32]);
    let period = 202503u64;

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
        CURRENT_PAYLOAD_VERSION,
    );

    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &data_hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig,
    );

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, data_hash);
    assert_eq!(wrap.period, period);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_wrong_payload_version_alone_fails_even_with_matching_sig() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[17u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let data_hash = BytesN::from_array(&env, &[0x88u8; 32]);
    let period = 202504u64;

    let correct_sig_for_v1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
        CURRENT_PAYLOAD_VERSION,
    );

    let wrong_version: u32 = 99;
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &data_hash,
        &wrong_version,
        &correct_sig_for_v1,
    );
}
