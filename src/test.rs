#![cfg(test)]

extern crate std;

use super::*;
use crate::test_utils::sign_payload;
use ed25519_dalek::SigningKey;
use crate::mint::MINT_SIGNATURE_PAYLOAD_VERSION;
use ed25519_dalek::{Signer, SigningKey};
use crate::mint::CURRENT_PAYLOAD_VERSION;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    Address, Bytes, BytesN, Env, String, Symbol, TryIntoVal,
};
use std::vec::Vec;

const STRESS_USER_COUNT: usize = 128;

extern crate std;

use super::*;
use ed25519_dalek::{Signer, SigningKey};
use crate::mint::CURRENT_PAYLOAD_VERSION;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    xdr::ToXdr,
    Address, Bytes, BytesN, Env, IntoVal, String, Symbol, TryIntoVal,
};
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::storage_types::{DataKey, WrapRecord};

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
    let mut payload = Bytes::new(env);
    payload.append(&Bytes::from_array(env, &[MINT_SIGNATURE_PAYLOAD_VERSION]));
    payload.append(&payload_version.to_xdr(env));
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

    // Global count starts at zero before any mint.
    assert_eq!(client.total_wrap_count(), 0);

    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[1u8; 32]);

    let events = env.events().all();
    let last_event = events.events().last().expect("no events found");
    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;
    let topics = &v0.topics;
    let data = &v0.data;

    let event_topic: Symbol = soroban_sdk::Val::try_from_val(&env, topics.get(0).unwrap()).unwrap().try_into_val(&env).unwrap();
    let event_user: Address = soroban_sdk::Val::try_from_val(&env, topics.get(1).unwrap()).unwrap().try_into_val(&env).unwrap();
    let event_period: u64 = soroban_sdk::Val::try_from_val(&env, topics.get(2).unwrap()).unwrap().try_into_val(&env).unwrap();
    let event_archetype: Symbol = data.try_into_val(&env).unwrap();

    let event_version: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let event_topic: Symbol = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let event_user: Address = topics.get(2).unwrap().try_into_val(&env).unwrap();
    let event_period: u64 = topics.get(3).unwrap().try_into_val(&env).unwrap();
    let event_archetype: Symbol = data.try_into_val(&env).unwrap();

    assert_eq!(event_version, symbol_short!("v1"));
    assert_eq!(event_topic, symbol_short!("mint"));
    assert_eq!(event_user, user);
    assert_eq!(event_period, period);
    assert_eq!(event_archetype, archetype);
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
    client.revoke_wrap(&user_a, &period_a);
    client.revoke_wrap(&user_b, &period_b);

    let events = env.events().all();

    let revoke_events: Vec<_> = events
        .iter()
        .filter(|(topic, _, _)| {
            let sym: Symbol = topic.get(0).unwrap().try_into_val(&env).unwrap();
            sym == symbol_short!("revoke")
        })
        .collect();

    assert_eq!(revoke_events.len(), 2);

    let (_, topics_a, data_a) = revoke_events[0];
    let event_user_a: Address = topics_a.get(1).unwrap().try_into_val(&env).unwrap();
    let event_period_a: u64 = topics_a.get(2).unwrap().try_into_val(&env).unwrap();
    let event_archetype_a: Symbol = data_a.try_into_val(&env).unwrap();
    assert_eq!(event_user_a, user_a);
    assert_eq!(event_period_a, period_a);
    assert_eq!(event_archetype_a, archetype_a);

    let (_, topics_b, data_b) = revoke_events[1];
    let event_user_b: Address = topics_b.get(1).unwrap().try_into_val(&env).unwrap();
    let event_period_b: u64 = topics_b.get(2).unwrap().try_into_val(&env).unwrap();
    let event_archetype_b: Symbol = data_b.try_into_val(&env).unwrap();
    assert_eq!(event_user_b, user_b);
    assert_eq!(event_period_b, period_b);
    assert_eq!(event_archetype_b, archetype_b);
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
    );
    client.mint_wrap(&user, &202402, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig2);
    client.mint_wrap(&user, &2022, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig2);

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
    );

    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);
    assert_eq!(client.total_revoked(), 0);

    client.revoke_wrap(&user, &period);

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
    client.revoke_wrap(&user, &202401);
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
    );
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

    let tampered_data = Bytes::from_slice(&env, b"{\"score\":999}");
    assert!(!client.verify_data(&user, &period, &tampered_data));
}


#[test]
fn test_contract_info_returns_correct_fields() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let info = client.contract_info();
    assert_eq!(info.name, String::from_str(&env, "Stellar Wrap Registry"));
    assert_eq!(info.version, String::from_str(&env, "0.1.0"));
    assert_eq!(
        info.repo,
        String::from_str(&env, "https://github.com/zintarh/stellar-wrap-contract")
    );
    assert_eq!(
        info.description,
        String::from_str(&env, "Soulbound token registry for Stellar Wrap")
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
    );
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

    let corrupted_data = Bytes::from_slice(&env, b"\x00\xFF\xFE\xFDcorrupt\x01\x02");
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
    client.mint_wrap(&user_a, &202401, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig_a);
    assert_eq!(client.total_wrap_count(), 1);
    assert_eq!(client.balance_of(&user_a), 1);

    // Second mint, same user, different period — global count goes up,
    // per-user count goes up too.
    let sig_a2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_a,
        202402,
        &archetype,
        &hash1,
     CURRENT_PAYLOAD_VERSION);
    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202404,
        &archetype,
        &hash2,
     CURRENT_PAYLOAD_VERSION);
    let sig3 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202403,
        &archetype,
        &hash3,
     CURRENT_PAYLOAD_VERSION);

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

    client.extend_ttl(&user, &9999);
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
    let sig_a = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_a,
        period,
        &archetype,
        &hash_a,
     CURRENT_PAYLOAD_VERSION);
    let sig_b = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
        &hash_b,
     CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user_a, &202402, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig_a2);
    assert_eq!(client.total_wrap_count(), 2);
    assert_eq!(client.balance_of(&user_a), 2);

    // Third mint, a different user entirely — global count still climbs,
    // confirming TotalWrapCount is contract-wide, not per-user.
    let user_b = Address::generate(&env);
    let sig_b = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_b,
        202401,
        &archetype,
        &lower_hash,
     CURRENT_PAYLOAD_VERSION);
    let upper_sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        210012,
        &archetype,
        &upper_hash,
     CURRENT_PAYLOAD_VERSION);

    client.mint_wrap(&user, &202401, &archetype, &lower_hash, &CURRENT_PAYLOAD_VERSION, &lower_sig);
    client.mint_wrap(&user, &210012, &archetype, &upper_hash, &CURRENT_PAYLOAD_VERSION, &upper_sig);

    assert!(client.get_wrap(&user, &202401).is_some());
    assert!(client.get_wrap(&user, &210012).is_some());
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_invalid_period_zero_fails() {
    client.mint_wrap(&user_a, &period, &archetype, &hash_a, &CURRENT_PAYLOAD_VERSION, &sig_a);
    client.mint_wrap(&user_b, &period, &archetype, &hash_b, &CURRENT_PAYLOAD_VERSION, &sig_b);

    let wrap_a = client.get_wrap(&user_a, &period).unwrap();
    let wrap_b = client.get_wrap(&user_b, &period).unwrap();
    assert_eq!(wrap_a.data_hash, hash_a);
    assert_eq!(wrap_b.data_hash, hash_b);
    assert_ne!(wrap_a.data_hash, wrap_b.data_hash);

    assert_eq!(client.balance_of(&user_a), 1);
    assert_eq!(client.balance_of(&user_b), 1);

    assert!(client.get_wrap(&user_a, &period).is_some());
    assert!(client.get_wrap(&user_b, &period).is_some());
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
    let period = 0u64;
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
     CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);

    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);
    let events = env.events().all();
    let last_event = events.last().expect("Expected at least one event");
    let (event_contract, topics, data) = last_event;

    assert_eq!(event_contract, contract_id);
    assert_eq!(topics.len(), 4, "Mint event must have exactly 4 topics");

    let topic_0: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic_1: Symbol = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let topic_2: Address = topics.get(2).unwrap().try_into_val(&env).unwrap();
    let topic_3: u64 = topics.get(3).unwrap().try_into_val(&env).unwrap();

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
     CURRENT_PAYLOAD_VERSION);

    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);

    let archetype_a = symbol_short!("builder");
    let archetype_b = symbol_short!("defi");
    let hash_a = BytesN::from_array(&env, &[10u8; 32]);
    let hash_b = BytesN::from_array(&env, &[20u8; 32]);
    let period_a = 202501u64;
    let period_b = 202502u64;

    let sig_a = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_a,
        period_a,
        &archetype_a,
        &hash_a,
     CURRENT_PAYLOAD_VERSION);
    let sig_b = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_b,
        period_b,
        &archetype_b,
        &hash_b,
     CURRENT_PAYLOAD_VERSION);

    client.mint_wrap(&user_a, &period_a, &archetype_a, &hash_a, &CURRENT_PAYLOAD_VERSION, &sig_a);
    client.mint_wrap(&user_b, &period_b, &archetype_b, &hash_b, &CURRENT_PAYLOAD_VERSION, &sig_b);

    let events = env.events().all();

    let mut mint_events = soroban_sdk::vec![&env];
    for event in events.iter() {
        let (addr, topics, _data) = &event;
        if *addr == contract_id && topics.len() == 4 {
            let t: Result<Symbol, _> = topics.get(1).unwrap().try_into_val(&env);
            if t.map_or(false, |s| s == symbol_short!("mint")) {
                mint_events.push_back(event.clone());
            }
        }
    }

    assert_eq!(mint_events.len(), 2, "Expected exactly 2 mint events");

    let (_, topics_a, data_a) = mint_events.get(0).unwrap();
    let ev_version: Symbol = topics_a.get(0).unwrap().try_into_val(&env).unwrap();
    let ev_user_a: Address = topics_a.get(2).unwrap().try_into_val(&env).unwrap();
    let ev_period_a: u64 = topics_a.get(3).unwrap().try_into_val(&env).unwrap();
    let ev_arch_a: Symbol = data_a.try_into_val(&env).unwrap();
    assert_eq!(ev_version, symbol_short!("v1"));
    assert_eq!(ev_user_a, user_a);
    assert_eq!(ev_period_a, period_a);
    assert_eq!(ev_arch_a, archetype_a);

    let (_, topics_b, data_b) = mint_events.get(1).unwrap();
    let ev_version: Symbol = topics_b.get(0).unwrap().try_into_val(&env).unwrap();
    let ev_user_b: Address = topics_b.get(2).unwrap().try_into_val(&env).unwrap();
    let ev_period_b: u64 = topics_b.get(3).unwrap().try_into_val(&env).unwrap();
    let ev_arch_b: Symbol = data_b.try_into_val(&env).unwrap();
    assert_eq!(ev_version, symbol_short!("v1"));
    assert_eq!(ev_user_b, user_b);
    assert_eq!(ev_period_b, period_b);
    assert_eq!(ev_arch_b, archetype_b);
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
        &data_hash,
     CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

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
         CURRENT_PAYLOAD_VERSION);

        client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);

        cpu_samples[i] = env.budget().cpu_instruction_cost();
        mem_samples[i] = env.budget().memory_bytes_cost();
        users.push(user);
    }
    let period = 2024u64;

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
     CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

    assert!(cpu_samples[0] > 0);
    assert!(mem_samples[0] > 0);
    assert!(cpu_samples.iter().all(|sample| *sample > 0));
    assert!(mem_samples.iter().all(|sample| *sample > 0));
    assert!(cpu_samples
        .iter()
        .skip(1)
        .any(|sample| *sample != cpu_samples[0]));
    assert!(mem_samples
        .iter()
        .skip(1)
        .any(|sample| *sample != mem_samples[0]));

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
fn test_get_mint_timestamp_exists() {
/// Verifies that `get_wrap` can be safely called before the contract is initialized.
/// 
/// Before initialization, no wrap records exist in persistent storage.
/// This test confirms that `get_wrap` returns `None` rather than panicking,
/// allowing client developers to query wrap state without requiring an
/// initialization guard.
#[test]
fn test_get_wrap_returns_none_before_initialization() {
// ─── Issue #26: instance storage TTL tests ──────────────────────────────────

#[test]
fn test_instance_ttl_extended_on_mint() {
    // Verifies that mint_wrap calls extend_ttl on instance storage,
    // keeping admin/pubkey/schema accessible after many ledgers.
// ─── Issue #25: update_admin_pubkey tests ───────────────────────────────────

#[test]
fn test_update_admin_pubkey_success() {
/// Issue #241: reminting after revoke must replace the archetype and emit both events.
#[test]
fn test_remint_after_revoke_updates_archetype() {
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
    let signing_key = SigningKey::from_bytes(&[14u8; 32]);
    let signing_key = SigningKey::from_bytes(&[95u8; 32]);
    let signing_key = SigningKey::from_bytes(&[21u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let dummy_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let signature = sign_payload(
    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        2022,
        &archetype,
        &hash1,
    );
    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &dummy_hash,
    );
    client.mint_wrap(&user, &period, &archetype, &dummy_hash, &CURRENT_PAYLOAD_VERSION, &signature);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(
        client.get_mint_timestamp(&user, &period),
        Some(wrap.timestamp)
        &hash2,
    );
    let sig3 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        2023,
        &archetype,
        &hash3,
    );
}

#[test]
fn test_get_mint_timestamp_missing() {
    let period = 202406u64;
    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[14u8; 32]);
    client.mint_wrap(&user, &2022, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);
    client.mint_wrap(&user, &2024, &archetype, &hash2, &CURRENT_PAYLOAD_VERSION, &sig2);
    client.mint_wrap(&user, &2023, &archetype, &hash3, &CURRENT_PAYLOAD_VERSION, &sig3);

    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);

    // After mint, admin is still readable — instance storage was not expired
    assert!(client.get_admin().is_some());
    assert_eq!(client.get_admin().unwrap(), admin);
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
    let old_signing_key = SigningKey::from_bytes(&[97u8; 32]);
    let old_pubkey = BytesN::from_array(&env, &old_signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &old_pubkey);
    env.mock_all_auths();

    // Mint with old key — should succeed
    let period_old = 202408u64;
    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[16u8; 32]);
    let sig_old = sign_payload(&env, &old_signing_key, &contract_id, &user, period_old, &archetype, &hash);
    client.mint_wrap(&user, &period_old, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig_old);

    // Rotate to new key
    let new_signing_key = SigningKey::from_bytes(&[98u8; 32]);
    let new_pubkey = BytesN::from_array(&env, &new_signing_key.verifying_key().to_bytes());
    client.update_admin_pubkey(&new_pubkey);
    let sig = sign_payload(
    let period = 202406u64;
    let archetype_old = symbol_short!("arch");
    let archetype_new = symbol_short!("builder");
    let hash_old = BytesN::from_array(&env, &[41u8; 32]);
    let hash_new = BytesN::from_array(&env, &[42u8; 32]);

    let sig_old = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);

    // Mint with new key — should succeed
    let period_new = 202409u64;
    let sig_new = sign_payload(&env, &new_signing_key, &contract_id, &user, period_new, &archetype, &hash);
    client.mint_wrap(&user, &period_new, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig_new);

    assert_eq!(client.balance_of(&user), 2);
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
    let signing_key = SigningKey::from_bytes(&[96u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let dummy_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

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

    client.mint_wrap(&user, &2024, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);
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
    );
    client.mint_wrap(&user, &period, &archetype, &hash_1, &CURRENT_PAYLOAD_VERSION, &sig_1);
    assert_eq!(client.balance_of(&user), 1);

    client.revoke_wrap(&user, &period);

    assert!(client.get_wrap(&user, &period).is_none());
    assert_eq!(client.balance_of(&user), 0);

    let events = env.events().all();
    let last_event = events.last().expect("Expected revoke event");
    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;
    let topics = &v0.topics;
    let data = &v0.data;

    let event_version: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let event_topic: Symbol = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let event_user: Address = topics.get(2).unwrap().try_into_val(&env).unwrap();
    let event_period: u64 = topics.get(3).unwrap().try_into_val(&env).unwrap();
    let revoked: bool = data.try_into_val(&env).unwrap();

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
    );
    client.mint_wrap(&user, &period, &archetype, &hash_2, &CURRENT_PAYLOAD_VERSION, &sig_2);

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

    client.revoke_wrap(&user, &2026);
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
        };
        env.storage().persistent().set(&wrap_key, &record);
        env.storage().persistent().set(&count_key, &1u32);
    });

    client.revoke_wrap(&user, &2026);
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
        &dummy_hash,
    );
    client.mint_wrap(&user, &period, &archetype, &dummy_hash, &CURRENT_PAYLOAD_VERSION, &signature);

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
        &data_hash,
    );

    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

    let guard_key = DataKey::MintGuard(user.clone());
    env.as_contract(&contract_id, || {
        assert!(!env.storage().temporary().has(&guard_key));
        assert!(!env.storage().persistent().has(&guard_key));
    });
}

#[test]
fn test_mint_guard_on_failure_leaves_no_residual_state() {
fn test_upgrade_emits_event() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[14u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let period = 2026u64;
    let archetype = symbol_short!("arch");
    let data_hash = BytesN::from_array(&env, &[14u8; 32]);
    let signature = sign_payload(
        &archetype_old,
        &hash_old,
    );
    client.mint_wrap(&user, &period, &archetype_old, &hash_old, &CURRENT_PAYLOAD_VERSION, &sig_old);

    let wrap_old = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap_old.archetype, archetype_old);

    client.revoke_wrap(&user, &period);
    assert!(client.get_wrap(&user, &period).is_none());

    let events_after_revoke = env.events().all();
    let (_, revoke_topics, revoke_data) = events_after_revoke
        .last()
        .expect("expected revoke event after revoke_wrap");
    let revoke_topic: Symbol = revoke_topics.get(0).unwrap().try_into_val(&env).unwrap();
    let revoke_user: Address = revoke_topics.get(1).unwrap().try_into_val(&env).unwrap();
    let revoke_period: u64 = revoke_topics.get(2).unwrap().try_into_val(&env).unwrap();
    let revoked: bool = revoke_data.try_into_val(&env).unwrap();
    assert_eq!(revoke_topic, symbol_short!("revoke"));
    assert_eq!(revoke_user, user);
    assert_eq!(revoke_period, period);
    assert!(revoked);

    let sig_new = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
    );

    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

    let duplicate = catch_unwind(AssertUnwindSafe(|| {
        client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature)
    }));
    assert!(duplicate.is_err());

    let guard_key = DataKey::MintGuard(user.clone());
    env.as_contract(&contract_id, || {
        assert!(!env.storage().temporary().has(&guard_key));
        assert!(!env.storage().persistent().has(&guard_key));
    });
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
    let last_event = events.last().expect("Expected at least one event");
    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;
    let topics = &v0.topics;
    let data = &v0.data;

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
    let period = 2024u64;

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &edge_hash,
    );
    client.mint_wrap(&user, &period, &archetype, &edge_hash, &CURRENT_PAYLOAD_VERSION, &sig);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, edge_hash);
}

#[test]
fn test_mint_wrap_max_hash_succeeds() {
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
#[should_panic(expected = "Error(Contract, #3)")]
fn test_unauthorized_upgrade_fails() {
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
    let period = 2024u64;

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &max_hash,
    );
    client.mint_wrap(&user, &period, &archetype, &max_hash, &CURRENT_PAYLOAD_VERSION, &sig);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, max_hash);
}

#[test]
#[should_panic]
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

    let event_topic: Symbol = soroban_sdk::Val::try_from_val(&env, topics.get(0).unwrap()).unwrap().try_into_val(&env).unwrap();
    let event_wasm_hash: BytesN<32> = data.try_into_val(&env).unwrap();

    assert_eq!(event_topic, symbol_short!("upgrade"));
    assert_eq!(event_wasm_hash, new_wasm_hash);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_upgrade_requires_admin_auth() {
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
    client.initialize(&admin, &pubkey);

    let fake_wasm = BytesN::from_array(&env, &[0u8; 32]);
    client.upgrade(&fake_wasm);
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
    )
}

#[test]
fn test_update_wrap_succeeds_and_preserves_timestamp() {
// ── Revocation Tests ──────────────────────────────────────────────────────────

#[test]
fn test_revoke_wrap_success() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let signing_key = SigningKey::from_bytes(&[30u8; 32]);
    let signing_key = SigningKey::from_bytes(&[15u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let dummy_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;

    let signature = sign_payload(
    let period = 2025u64;
    let archetype = symbol_short!("arch");
    let hash1 = BytesN::from_array(&env, &[41u8; 32]);

    let sig1 = sign_payload(
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
        &hash1,
    );
    client.mint_wrap(&user, &period, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);

    let before = client.get_wrap(&user, &period).unwrap();

    let new_hash = BytesN::from_array(&env, &[99u8; 32]);
    let new_arch = symbol_short!("builder");
    let sig2 = sign_update_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &new_arch,
        &new_hash,
    );
    client.update_wrap(&user, &period, &new_hash, &new_arch, &sig2);

    let after = client.get_wrap(&user, &period).unwrap();
    assert_eq!(
        after.timestamp, before.timestamp,
        "Original timestamp must be preserved"
    );
    assert_eq!(after.data_hash, new_hash);
    assert_eq!(after.archetype, new_arch);
    assert_eq!(after.period, period);
}

#[test]
fn test_update_wrap_emits_update_event() {
        &hash,
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

    let signing_key = SigningKey::from_bytes(&[31u8; 32]);
    let signing_key = SigningKey::from_bytes(&[16u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let period = 2025u64;
    let archetype = symbol_short!("arch");
    let hash1 = BytesN::from_array(&env, &[41u8; 32]);
    let sig1 = sign_payload(
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
        &hash1,
    );
    client.mint_wrap(&user, &period, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);

    let new_hash = BytesN::from_array(&env, &[98u8; 32]);
    let new_arch = symbol_short!("defi");
    let sig2 = sign_update_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &new_arch,
        &new_hash,
    );
    client.update_wrap(&user, &period, &new_hash, &new_arch, &sig2);

    let events = env.events().all();
    let last_event = events.last().unwrap();
    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;
    let topics = &v0.topics;
    let data = &v0.data;

    let topic_0: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic_1: Symbol = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let topic_2: Address = topics.get(2).unwrap().try_into_val(&env).unwrap();
    let topic_3: u64 = topics.get(3).unwrap().try_into_val(&env).unwrap();
    let ev_arch: Symbol = data.try_into_val(&env).unwrap();

    assert_eq!(topic_0, symbol_short!("v1"));
    assert_eq!(topic_1, symbol_short!("update"));
    assert_eq!(topic_2, user);
    assert_eq!(topic_3, period);
    assert_eq!(ev_arch, new_arch);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_update_wrap_nonexistent_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[32u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let new_hash = BytesN::from_array(&env, &[99u8; 32]);
    let new_arch = symbol_short!("arch");
    let sig = sign_update_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        9999,
        &new_arch,
        &new_hash,
    );
    client.update_wrap(&user, &9999, &new_hash, &new_arch, &sig);
}

#[test]
#[should_panic]
fn test_update_wrap_requires_admin_auth() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[33u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let period = 2025u64;
    let archetype = symbol_short!("arch");
    let hash1 = BytesN::from_array(&env, &[41u8; 32]);
    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash1,
    );
    client.mint_wrap(&user, &period, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);

    let env2 = Env::default();
    let contract_id2 = env2.register_contract(None, StellarWrapContract);
    let client2 = StellarWrapContractClient::new(&env2, &contract_id2);
    client2.initialize(&admin, &admin_pubkey);

    let new_hash = BytesN::from_array(&env2, &[99u8; 32]);
    let new_arch = symbol_short!("arch");
    let sig2 = sign_update_payload(
        &env2,
        &signing_key,
        &contract_id2,
        &user,
        period,
        &new_arch,
        &new_hash,
    );
    client2.update_wrap(&user, &period, &new_hash, &new_arch, &sig2);
}

#[test]
#[should_panic]
fn test_update_wrap_zero_hash_rejected() {
        &hash,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);

    let reason_hash = BytesN::from_array(&env, &[0xAAu8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    let events = env.events().all();
    let last_event = events.events().last().expect("no events found");
    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;
    let topics = &v0.topics;
    let data = &v0.data;

    let event_topic: Symbol = soroban_sdk::Val::try_from_val(&env, topics.get(0).unwrap()).unwrap().try_into_val(&env).unwrap();
    let event_user: Address = soroban_sdk::Val::try_from_val(&env, topics.get(1).unwrap()).unwrap().try_into_val(&env).unwrap();
    let event_period: u64 = soroban_sdk::Val::try_from_val(&env, topics.get(2).unwrap()).unwrap().try_into_val(&env).unwrap();
    let event_reason: BytesN<32> = data.try_into_val(&env).unwrap();

    assert_eq!(event_topic, symbol_short!("revoke"));
    assert_eq!(event_user, user);
    assert_eq!(event_period, period);
    assert_eq!(event_reason, reason_hash);
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

    let period = 2025u64;
    let archetype = symbol_short!("arch");
    let hash1 = BytesN::from_array(&env, &[41u8; 32]);
    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash1,
    );
    client.mint_wrap(&user, &period, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);

    let zero_hash = BytesN::from_array(&env, &[0u8; 32]);
    let sig2 = sign_update_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &dummy_hash,
    );
    client.mint_wrap(&user, &period, &archetype, &dummy_hash, &CURRENT_PAYLOAD_VERSION, &signature);

    // Transition Active -> Draft is invalid and should fail with #8 (InvalidStateTransition)
    client.transition_wrap_state(&user, &period, &WrapState::Draft);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_fsm_transition_nonexistent_wrap_fails() {
    let old_signing_key = SigningKey::from_bytes(&[99u8; 32]);
    let old_pubkey = BytesN::from_array(&env, &old_signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &old_pubkey);
    env.mock_all_auths();

    // Rotate to a new pubkey
    let new_signing_key = SigningKey::from_bytes(&[100u8; 32]);
    let new_pubkey = BytesN::from_array(&env, &new_signing_key.verifying_key().to_bytes());
    client.update_admin_pubkey(&new_pubkey);

    // Attempt to mint with the OLD key — should fail because old sig doesn't verify
    // against the new pubkey. Soroban's ed25519_verify raises Error(Crypto, InvalidInput).
    let period = 202410u64;
    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[17u8; 32]);
    let sig_old = sign_payload(&env, &old_signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig_old);
        &zero_hash,
    );
    client.update_wrap(&user, &period, &zero_hash, &archetype, &sig2);
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn test_update_admin_pubkey_requires_admin_auth() {
    let user = Address::generate(&env);
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
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    let user = Address::generate(&env);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    client.transition_wrap_state(&user, &202401u64, &WrapState::Archived);
}

    let user = Address::generate(&env);
    let period = 202401u64;

    let result = client.get_wrap(&user, &period);
    assert!(result.is_none());
    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[15u8; 32]);

    let sig_a = sign_payload(&env, &signing_key, &contract_id, &user_a, 202407u64, &archetype, &hash);
    client.mint_wrap(&user_a, &202407u64, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig_a);

    let sig_b = sign_payload(&env, &signing_key, &contract_id, &user_b, 202407u64, &archetype, &hash);
    client.mint_wrap(&user_b, &202407u64, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig_b);

    // Admin address still accessible after multiple mints
    assert_eq!(client.get_schema_version(), 1);
    assert_eq!(client.get_admin().unwrap(), admin);
}

// ─── Issue #91: TTL auto-renewal for active users ────────────────────────────

#[test]
fn test_metadata_ttl_extended_on_new_mint() {
    let signing_key = SigningKey::from_bytes(&[101u8; 32]);
    let pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &pubkey);
    // Do NOT mock_all_auths — calling without admin authorization should fail

    let new_pubkey = BytesN::from_array(&env, &[0u8; 32]);
    client.update_admin_pubkey(&new_pubkey);
    let non_admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);

    // Mock only the non-admin authorization, admin should not be authorized
    env.mock_all_auths_allowing_non_root_auth();

    let new_wasm_hash = BytesN::from_array(&env, &[42u8; 32]);
    // This should panic because non_admin is not the admin
    client.upgrade(&new_wasm_hash);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_update_admin_pubkey_before_init_fails() {
    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    let user = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);

    client.revoke_wrap(&user, &202401, &reason_hash);
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
    let pubkey = BytesN::from_array(&env, &[2u8; 32]);
    client.initialize(&admin, &pubkey);

    let user = Address::generate(&env);
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

    let signing_key = SigningKey::from_bytes(&[50u8; 32]);
    let signing_key = SigningKey::from_bytes(&[17u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[1u8; 32]);

    // Mint first wrap
    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, 2024, &archetype, &hash);
    client.mint_wrap(&user, &2024, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig1);

    // After first mint: balance = 1, latest = 2024
    assert_eq!(client.balance_of(&user), 1);
    assert_eq!(client.get_latest_wrap(&user).unwrap().period, 2024);

    // Mint second wrap (higher period)
    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 2025, &archetype, &hash);
    client.mint_wrap(&user, &2025, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig2);

    // After second mint: balance = 2, latest = 2025
    assert_eq!(client.balance_of(&user), 2);
    assert_eq!(client.get_latest_wrap(&user).unwrap().period, 2025);

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
    env.mock_all_auths();
    let new_pubkey = BytesN::from_array(&env, &[0u8; 32]);
    client.update_admin_pubkey(&new_pubkey);
}

#[test]
fn test_update_admin_pubkey_emits_event() {
    let hash1 = BytesN::from_array(&env, &[10u8; 32]);
    let hash2 = BytesN::from_array(&env, &[20u8; 32]);

    let sig1 = sign_payload(
        &env, &signing_key, &contract_id, &user, 202401, &archetype, &hash1,
    );
    let sig2 = sign_payload(
        &env, &signing_key, &contract_id, &user, 202402, &archetype, &hash2,
    );

    client.mint_wrap(&user, &202401, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);
    client.mint_wrap(&user, &202402, &archetype, &hash2, &CURRENT_PAYLOAD_VERSION, &sig2);

    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, 202402);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &202402, &reason_hash);

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
    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, 2024, &archetype, &hash1);
    client.mint_wrap(&user, &2024, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);

    // Mint second wrap (period 2025)
    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 2025, &archetype, &hash2);
    client.mint_wrap(&user, &2025, &archetype, &hash2, &CURRENT_PAYLOAD_VERSION, &sig2);

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
    let sig1 = sign_payload(
        &env, &signing_key, &contract_id, &user, 202401, &archetype, &hash1,
    );
    let sig2 = sign_payload(
        &env, &signing_key, &contract_id, &user, 202403, &archetype, &hash2,
    );

    client.mint_wrap(&user, &202401, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);
    client.mint_wrap(&user, &202403, &archetype, &hash2, &CURRENT_PAYLOAD_VERSION, &sig2);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &202401, &reason_hash);

    // Latest period (202403) should still be retrievable
    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, 202403);
    assert_eq!(client.balance_of(&user), 1);
}

#[test]
fn test_remint_after_revoke_succeeds() {
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
    // Ensure they are distinct
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

    let signing_key = SigningKey::from_bytes(&[52u8; 32]);
    let signing_key = SigningKey::from_bytes(&[19u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let archetype = symbol_short!("arch");

    let sig = sign_payload(&env, &signing_key, &contract_id, &user, 2024, &archetype, &hash);
    client.mint_wrap(&user, &2024, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);

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
    let archetype = symbol_short!("arch");
    let hash1 = BytesN::from_array(&env, &[10u8; 32]);
    let hash2 = BytesN::from_array(&env, &[20u8; 32]);
    let period = 202401u64;

    let sig1 = sign_payload(
        &env, &signing_key, &contract_id, &user, period, &archetype, &hash1,
    );
    client.mint_wrap(&user, &period, &archetype, &hash1, &CURRENT_PAYLOAD_VERSION, &sig1);

    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    // Should be able to mint a new wrap for the same period after revocation
    let sig2 = sign_payload(
        &env, &signing_key, &contract_id, &user, period, &archetype, &hash2,
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
    let signing_key = SigningKey::from_bytes(&[102u8; 32]);
    let pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    let new_pubkey = BytesN::from_array(&env, &[5u8; 32]);
    client.update_admin_pubkey(&new_pubkey);

    let events = env.events().all();
    let last_event = events.last().expect("No events found");
    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;
    let topics = &v0.topics;
    let data = &v0.data;

    let t0: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let t1: Symbol = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let emitted_key: BytesN<32> = data.try_into_val(&env).unwrap();

    assert_eq!(t0, symbol_short!("admin"));
    assert_eq!(t1, symbol_short!("pubkey"));
    assert_eq!(emitted_key, new_pubkey);
fn test_upgrade_before_init_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    env.mock_all_auths();

    let new_wasm_hash = BytesN::from_array(&env, &[42u8; 32]);
    client.upgrade(&new_wasm_hash);
}
    client.mint_wrap(&user_b, &202401, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig_b);
    assert_eq!(client.total_wrap_count(), 3);
    assert_eq!(client.balance_of(&user_b), 1);
}

// NOTE: This issue's acceptance criteria also calls for decrement-on-revoke
// and remint tracking/tests. As of this change the contract has no
// revoke/remint capability at all (no revoke_wrap function exists anywhere
// in the codebase) — only mint_wrap. TotalWrapCount is implemented as a
// simple increment-on-mint counter so that decrementing it is a one-line
// change (mirroring this same read-increment-write pattern) whenever
// revoke_wrap is built. Flagged on the issue for a scope decision on
// whether revoke belongs in this PR or a follow-up.
        &archetype_new,
        &hash_new,
     CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user, &period, &archetype_new, &hash_new, &CURRENT_PAYLOAD_VERSION, &sig_new);

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
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;
    let signature = sign_payload(
        &env, &signing_key, &contract_id, &user, period, &archetype, &hash,
     CURRENT_PAYLOAD_VERSION);
    client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &signature);

    // Zero reason hash (no reason provided)
    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason_hash);

    assert!(client.get_wrap(&user, &period).is_none());
    assert_eq!(client.balance_of(&user), 0);

    // Verify the event still emitted with zero hash
    let events = env.events().all();
    let last_event = events.events().last().expect("no events found");
    let (_, _, data) = last_event;
    let event_reason: BytesN<32> = data.try_into_val(&env).unwrap();
    assert_eq!(event_reason, reason_hash);
fn test_set_alias_hash_requires_auth() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    // Do NOT call env.mock_all_auths() — auth must be required
    let user = Address::generate(&env);
    let alias_hash = BytesN::from_array(&env, &[0xccu8; 32]);

    client.set_alias_hash(&user, &alias_hash);
    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);

    let dummy_wasm_hash = BytesN::from_array(&env, &[0xBBu8; 32]);
    client.upgrade(&dummy_wasm_hash);
}

// ─── Feature #287: User opt-out flag ────────────────────────────────────────

#[cfg(test)]
mod opt_out_tests {
    extern crate std;

    use ed25519_dalek::{Signer, SigningKey};
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, MockAuth, MockAuthInvoke},
        Address, BytesN, Env,
    };

    use crate::{
        mint::CURRENT_PAYLOAD_VERSION,
        signature::construct_mint_payload,
        StellarWrapContract, StellarWrapContractClient,
    };

    /// Build a valid admin Ed25519 signature for a mint call.
    fn make_sig(
        env: &Env,
        signer: &SigningKey,
        contract_id: &Address,
        user: &Address,
        period: u64,
        archetype: &soroban_sdk::Symbol,
        data_hash: &BytesN<32>,
    ) -> BytesN<64> {
        let payload = construct_mint_payload(
            env,
            contract_id,
            user,
            period,
            archetype,
            data_hash,
            CURRENT_PAYLOAD_VERSION,
        );
        let mut buf = [0u8; 512];
        let len = payload.len() as usize;
        payload.copy_into_slice(&mut buf[..len]);
        let sig = signer.sign(&buf[..len]);
        BytesN::from_array(env, &sig.to_bytes())
    }

    // ── is_opted_out defaults to false ──────────────────────────────────────

    #[test]
    fn test_is_opted_out_default_false() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let client = StellarWrapContractClient::new(&env, &contract_id);

        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
        let admin = Address::generate(&env);
        client.initialize(&admin, &admin_pubkey);

        let user = Address::generate(&env);
        assert!(!client.is_opted_out(&user));
    }

    // ── opt_out sets the flag ────────────────────────────────────────────────

    #[test]
    fn test_opt_out_sets_flag() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let client = StellarWrapContractClient::new(&env, &contract_id);

        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
        let admin = Address::generate(&env);
        client.initialize(&admin, &admin_pubkey);
        env.mock_all_auths();

        let user = Address::generate(&env);
        client.opt_out(&user);
        assert!(client.is_opted_out(&user));
    }

    // ── opt_in clears the flag ───────────────────────────────────────────────

    #[test]
    fn test_opt_in_clears_flag() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let client = StellarWrapContractClient::new(&env, &contract_id);

        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
        let admin = Address::generate(&env);
        client.initialize(&admin, &admin_pubkey);
        env.mock_all_auths();

        let user = Address::generate(&env);
        client.opt_out(&user);
        assert!(client.is_opted_out(&user));

        client.opt_in(&user);
        assert!(!client.is_opted_out(&user));
    }

    // ── opted-out user cannot have a wrap minted (error #15) ─────────────────

    #[test]
    #[should_panic(expected = "Error(Contract, #15)")]
    fn test_opted_out_user_mint_rejected() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let client = StellarWrapContractClient::new(&env, &contract_id);

        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
        let admin = Address::generate(&env);
        client.initialize(&admin, &admin_pubkey);
        env.mock_all_auths();

        let user = Address::generate(&env);
        let archetype = symbol_short!("arch");
        let hash = BytesN::from_array(&env, &[1u8; 32]);
        let period = 202501u64;

        client.opt_out(&user);

        let sig = make_sig(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
        client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);
    }

    // ── after opt-in, minting succeeds again ─────────────────────────────────

    #[test]
    fn test_opt_in_re_enables_mint() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let client = StellarWrapContractClient::new(&env, &contract_id);

        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
        let admin = Address::generate(&env);
        client.initialize(&admin, &admin_pubkey);
        env.mock_all_auths();

        let user = Address::generate(&env);
        let archetype = symbol_short!("arch");
        let hash = BytesN::from_array(&env, &[2u8; 32]);
        let period = 202502u64;

        // Opt out then back in.
        client.opt_out(&user);
        client.opt_in(&user);
        assert!(!client.is_opted_out(&user));

        let sig = make_sig(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
        client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);
        assert!(client.get_wrap(&user, &period).is_some());
    }

    // ── opt_out requires the user's own auth ─────────────────────────────────

    #[test]
    #[should_panic]
    fn test_opt_out_requires_user_auth() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let client = StellarWrapContractClient::new(&env, &contract_id);

        let signing_key = SigningKey::from_bytes(&[8u8; 32]);
        let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
        let admin = Address::generate(&env);
        client.initialize(&admin, &admin_pubkey);
        // No mock_all_auths — auth must be required.

        let user = Address::generate(&env);
        client.opt_out(&user); // must panic — no auth provided
    }

    // ── opt_in requires the user's own auth ──────────────────────────────────

    #[test]
    #[should_panic]
    fn test_opt_in_requires_user_auth() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let client = StellarWrapContractClient::new(&env, &contract_id);

        let signing_key = SigningKey::from_bytes(&[9u8; 32]);
        let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
        let admin = Address::generate(&env);
        client.initialize(&admin, &admin_pubkey);
        // No mock_all_auths — auth must be required.

        let user = Address::generate(&env);
        client.opt_in(&user); // must panic — no auth provided
    }

    // ── admin auth alone cannot satisfy opt_out for a different user ─────────

    #[test]
    #[should_panic]
    fn test_admin_cannot_opt_out_another_user() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let client = StellarWrapContractClient::new(&env, &contract_id);

        let signing_key = SigningKey::from_bytes(&[10u8; 32]);
        let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
        let admin = Address::generate(&env);
        client.initialize(&admin, &admin_pubkey);

        // Only mock the admin's auth, not the user's.
        env.mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "opt_out",
                args: soroban_sdk::vec![&env].into(),
            },
        }]);

        let user = Address::generate(&env);
        client.opt_out(&user); // must panic — admin auth != user auth
    }

    // ── non-opted-out user can mint normally ────────────────────────────────

    #[test]
    fn test_mint_succeeds_when_not_opted_out() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let client = StellarWrapContractClient::new(&env, &contract_id);

        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
        let admin = Address::generate(&env);
        client.initialize(&admin, &admin_pubkey);
        env.mock_all_auths();

        let user = Address::generate(&env);
        let archetype = symbol_short!("arch");
        let hash = BytesN::from_array(&env, &[3u8; 32]);
        let period = 202503u64;

        assert!(!client.is_opted_out(&user));

        let sig = make_sig(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
        client.mint_wrap(&user, &period, &archetype, &hash, &CURRENT_PAYLOAD_VERSION, &sig);
        assert!(client.get_wrap(&user, &period).is_some());
    }

    // ── multiple opt-out / opt-in cycles are idempotent ─────────────────────

    #[test]
    fn test_multiple_opt_cycles_are_idempotent() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let client = StellarWrapContractClient::new(&env, &contract_id);

        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
        let admin = Address::generate(&env);
        client.initialize(&admin, &admin_pubkey);
        env.mock_all_auths();

        let user = Address::generate(&env);

        // Double opt-out does not change behavior.
        client.opt_out(&user);
        client.opt_out(&user);
        assert!(client.is_opted_out(&user));

        // Double opt-in does not change behavior.
        client.opt_in(&user);
        client.opt_in(&user);
        assert!(!client.is_opted_out(&user));
    }
}
