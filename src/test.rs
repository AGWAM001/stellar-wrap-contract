#![cfg(test)]
#![allow(deprecated)]

extern crate std;

use super::*;
use crate::mint::CURRENT_PAYLOAD_VERSION;
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    xdr::ToXdr,
    Address, Bytes, BytesN, Env, String, Symbol,
};

/// Signs the same payload layout the contract rebuilds in signature::construct_mint_payload.
#[allow(clippy::too_many_arguments)]
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
    payload.append(&Bytes::from_array(
        env,
        crate::signature::MINT_DOMAIN_SEPARATOR,
    ));
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

// ─── Initialization tests ────────────────────────────────────────────────────

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
    assert!(!health.initialized);
    assert!(!health.has_admin);
    assert!(!health.has_signing_key);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);

    let health = client.health();
    assert!(health.initialized);
    assert!(health.has_admin);
    assert!(health.has_signing_key);
}

#[test]
fn test_get_admin_before_init_returns_none() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    assert!(client.get_admin().is_none());
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
#[should_panic(expected = "Error(Contract, #2)")]
fn test_update_admin_before_init_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    env.mock_all_auths();

    let new_admin = Address::generate(&env);
    client.update_admin(&new_admin);
}

// ─── Token metadata tests ────────────────────────────────────────────────────

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

// ─── Mint tests ──────────────────────────────────────────────────────────────

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
    // Second mint for same user+period should panic
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
fn test_total_wrap_count_increments() {
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
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    let sig_a = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_a,
        202401,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user_a,
        &202401,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig_a,
    );
    assert_eq!(client.total_wrap_count(), 1);

    let sig_b = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_b,
        202401,
        &archetype,
        &hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &user_b,
        &202401,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &sig_b,
    );
    assert_eq!(client.total_wrap_count(), 2);
}

// ─── Verify data tests ───────────────────────────────────────────────────────

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

// ─── Revoke tests ────────────────────────────────────────────────────────────

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

    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );
    assert_eq!(client.total_revoked(), 0);

    let reason = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &period, &reason);

    assert_eq!(client.total_revoked(), 1);
    assert_eq!(client.balance_of(&user), 0);
    assert!(client.get_wrap(&user, &period).is_none());
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

    let reason = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &2026, &reason);
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
    let reason = BytesN::from_array(&env, &[0xAAu8; 32]);

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

    client.revoke_wrap(&user, &period, &reason);

    assert!(client.get_wrap(&user, &period).is_none());
    assert_eq!(client.balance_of(&user), 0);

    // Remint after revoke
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

// ─── Get wrap / query tests ──────────────────────────────────────────────────

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

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.period, period);
    assert_eq!(wrap.data_hash, hash);

    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, period);
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

    client.mint_wrap(
        &user,
        &202402,
        &archetype,
        &hash1,
        &CURRENT_PAYLOAD_VERSION,
        &sig1,
    );
    client.mint_wrap(
        &user,
        &202404,
        &archetype,
        &hash2,
        &CURRENT_PAYLOAD_VERSION,
        &sig2,
    );
    client.mint_wrap(
        &user,
        &202403,
        &archetype,
        &hash3,
        &CURRENT_PAYLOAD_VERSION,
        &sig3,
    );

    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, 202404);
    assert_eq!(latest.data_hash, hash2);
}

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

// ─── Migration tests ─────────────────────────────────────────────────────────

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

// ─── Mint timestamp tests ────────────────────────────────────────────────────

#[test]
fn test_get_mint_timestamp_exists() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[95u8; 32]);
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
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );

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

// ─── Get wraps pagination test ───────────────────────────────────────────────

#[test]
fn test_get_wraps_paginated() {
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

    // Mint multiple wraps
    for period in &[202401u64, 202402u64, 202403u64] {
        let sig = sign_payload(
            &env,
            &signing_key,
            &contract_id,
            &user,
            *period,
            &archetype,
            &hash,
            CURRENT_PAYLOAD_VERSION,
        );
        client.mint_wrap(
            &user,
            period,
            &archetype,
            &hash,
            &CURRENT_PAYLOAD_VERSION,
            &sig,
        );
    }

    // Paginated query
    let page1 = client.get_wraps(&user, &0, &2);
    assert_eq!(page1.len(), 2);

    let page2 = client.get_wraps(&user, &2, &2);
    assert_eq!(page2.len(), 1);
}

// ─── Alias hash tests ────────────────────────────────────────────────────────

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
    assert!(client.get_alias_hash(&user).is_none());

    env.mock_all_auths();
    client.set_alias_hash(&user, &alias_hash);

    assert_eq!(client.get_alias_hash(&user).unwrap(), alias_hash);
}

#[test]
fn test_get_alias_hash_returns_none_for_unknown_user() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);

    let unknown_user = Address::generate(&env);
    assert!(client.get_alias_hash(&unknown_user).is_none());
}

// ─── Pause / Unpause tests ───────────────────────────────────────────────────

#[test]
fn test_pause_and_unpause() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    assert!(!client.is_paused());

    client.pause();
    assert!(client.is_paused());

    client.unpause();
    assert!(!client.is_paused());
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_mint_wrap_fails_when_paused() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[22u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    client.pause();

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
}

// ─── Storage fee tests ───────────────────────────────────────────────────────

#[test]
fn test_storage_bytes_starts_at_zero() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);

    assert_eq!(client.storage_bytes(), 0);
}

#[test]
fn test_storage_bytes_increases_after_mint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[23u8; 32]);
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

    assert!(client.storage_bytes() > 0);
}

// ─── Fee params tests ────────────────────────────────────────────────────────

#[test]
fn test_fee_params_default_and_set() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    let default = client.fee_params();
    assert_eq!(default.base_fee, 0);
    assert_eq!(default.per_kib_fee, 0);

    use crate::storage_types::FeeParams;
    let new_params = FeeParams {
        base_fee: 100,
        per_kib_fee: 10,
        scale_step_kib: 1024,
        max_fee: 10000,
    };
    client.set_fee_params(&new_params);

    let retrieved = client.fee_params();
    assert_eq!(retrieved.base_fee, 100);
    assert_eq!(retrieved.per_kib_fee, 10);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Issue #490: Integration test simulating a complete user journey
// (init → mint → verify)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_complete_user_journey_init_mint_verify() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    // ── Step 1: Set up admin keypair ────────────────────────────────────────
    let signing_key = SigningKey::from_bytes(&[99u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    // ── Step 2: Initialize the contract ─────────────────────────────────────
    client.initialize(&admin, &admin_pubkey);

    // Verify initialization succeeded
    let health = client.health();
    assert!(health.initialized);
    assert_eq!(client.get_admin().unwrap(), admin);

    env.mock_all_auths();

    // ── Step 3: Set up the ledger timestamp ────────────────────────
    env.ledger().with_mut(|li| {
        li.timestamp = 1_700_000_000;
    });

    // ── Step 4: Prepare wrap minting data ───────────────────────────────────
    let user = Address::generate(&env);
    let period: u64 = 202506;
    let archetype = symbol_short!("builder");

    let data_json = Bytes::from_slice(
        &env,
        b"{\"repo\":\"stellar-wrap\",\"commits\":42,\"rating\":\"gold\"}",
    );
    let data_hash_raw = env.crypto().sha256(&data_json);
    let data_hash = BytesN::from_array(&env, &data_hash_raw.to_array());

    // Sign the mint payload with the admin key
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

    // ── Step 5: Mint the wrap ───────────────────────────────────────────────
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &data_hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );

    // ── Step 6: Verify the minted wrap via get_wrap ─────────────────────────
    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.period, period);
    assert_eq!(wrap.data_hash, data_hash);
    assert_eq!(wrap.archetype, archetype);
    assert_eq!(wrap.timestamp, 1_700_000_000);

    // ── Step 7: Verify via balance_of ───────────────────────────────────────
    assert_eq!(client.balance_of(&user), 1);

    // ── Step 8: Verify via total_wrap_count ─────────────────────────────────
    assert_eq!(client.total_wrap_count(), 1);

    // ── Step 9: Verify via verify_data (matching data) ──────────────────────
    assert!(client.verify_data(&user, &period, &data_json));

    // ── Step 10: Verify via verify_data (tampered data fails) ────────────────
    let tampered = Bytes::from_slice(&env, b"{\"repo\":\"stellar-wrap\",\"commits\":99}");
    assert!(!client.verify_data(&user, &period, &tampered));

    // ── Step 11: Verify get_latest_wrap returns the minted wrap ─────────────
    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, period);
    assert_eq!(latest.data_hash, data_hash);

    // ── Step 12: Verify get_mint_timestamp returns the correct timestamp ────
    assert_eq!(
        client.get_mint_timestamp(&user, &period),
        Some(wrap.timestamp)
    );

    // ── Step 13: Verify paginated get_wraps ─────────────────────────────────
    let wraps = client.get_wraps(&user, &0, &10);
    assert_eq!(wraps.len(), 1);
    assert_eq!(wraps.get(0).unwrap().period, period);

    // ── Step 14: Mint a second wrap and verify both coexist ─────────────────
    let period2: u64 = 202507;
    let data_json2 = Bytes::from_slice(
        &env,
        b"{\"repo\":\"stellar-wrap\",\"commits\":50,\"rating\":\"platinum\"}",
    );
    let data_hash_raw2 = env.crypto().sha256(&data_json2);
    let data_hash2 = BytesN::from_array(&env, &data_hash_raw2.to_array());

    let signature2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period2,
        &archetype,
        &data_hash2,
        CURRENT_PAYLOAD_VERSION,
    );

    client.mint_wrap(
        &user,
        &period2,
        &archetype,
        &data_hash2,
        &CURRENT_PAYLOAD_VERSION,
        &signature2,
    );

    // Both wraps accessible
    assert!(client.get_wrap(&user, &period).is_some());
    assert!(client.get_wrap(&user, &period2).is_some());
    assert_eq!(client.balance_of(&user), 2);
    assert_eq!(client.total_wrap_count(), 2);

    // Latest wrap is the second one
    let latest2 = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest2.period, period2);
    assert_eq!(latest2.data_hash, data_hash2);

    // Paginated query returns both
    let wraps2 = client.get_wraps(&user, &0, &10);
    assert_eq!(wraps2.len(), 2);
}

#[test]
fn test_user_journey_mint_then_revoke_then_remint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[100u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let user = Address::generate(&env);
    let period: u64 = 202508;
    let archetype = symbol_short!("audit");
    let reason = BytesN::from_array(&env, &[0xDEu8; 32]);

    let data = Bytes::from_slice(&env, b"{\"audit\":\"passed\"}");
    let hash_raw = env.crypto().sha256(&data);
    let hash = BytesN::from_array(&env, &hash_raw.to_array());

    // Mint
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

    assert_eq!(client.balance_of(&user), 1);
    assert!(client.verify_data(&user, &period, &data));

    // Revoke
    client.revoke_wrap(&user, &period, &reason);
    assert_eq!(client.balance_of(&user), 0);
    assert_eq!(client.total_revoked(), 1);
    assert!(client.get_wrap(&user, &period).is_none());
    assert!(!client.verify_data(&user, &period, &data));

    // Remint with different data
    let data2 = Bytes::from_slice(&env, b"{\"audit\":\"repassed\"}");
    let hash_raw2 = env.crypto().sha256(&data2);
    let hash2 = BytesN::from_array(&env, &hash_raw2.to_array());

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

    assert_eq!(client.balance_of(&user), 1);
    assert!(client.verify_data(&user, &period, &data2));
    assert!(!client.verify_data(&user, &period, &data)); // old data no longer matches
}

#[test]
fn test_user_journey_multiple_users_independent_wraps() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[101u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("social");
    let period: u64 = 202509;
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let alice_data = Bytes::from_slice(&env, b"{\"user\":\"alice\",\"score\":95}");
    let alice_hash_raw = env.crypto().sha256(&alice_data);
    let alice_hash = BytesN::from_array(&env, &alice_hash_raw.to_array());

    let bob_data = Bytes::from_slice(&env, b"{\"user\":\"bob\",\"score\":87}");
    let bob_hash_raw = env.crypto().sha256(&bob_data);
    let bob_hash = BytesN::from_array(&env, &bob_hash_raw.to_array());

    // Mint for Alice
    let alice_sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &alice,
        period,
        &archetype,
        &alice_hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &alice,
        &period,
        &archetype,
        &alice_hash,
        &CURRENT_PAYLOAD_VERSION,
        &alice_sig,
    );

    // Mint for Bob
    let bob_sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &bob,
        period,
        &archetype,
        &bob_hash,
        CURRENT_PAYLOAD_VERSION,
    );
    client.mint_wrap(
        &bob,
        &period,
        &archetype,
        &bob_hash,
        &CURRENT_PAYLOAD_VERSION,
        &bob_sig,
    );

    // Each user sees only their data
    assert_eq!(client.balance_of(&alice), 1);
    assert_eq!(client.balance_of(&bob), 1);
    assert_eq!(client.total_wrap_count(), 2);

    assert!(client.verify_data(&alice, &period, &alice_data));
    assert!(client.verify_data(&bob, &period, &bob_data));
    assert!(!client.verify_data(&alice, &period, &bob_data));
    assert!(!client.verify_data(&bob, &period, &alice_data));

    // Independent get_wrap
    assert_eq!(
        client.get_wrap(&alice, &period).unwrap().data_hash,
        alice_hash
    );
    assert_eq!(client.get_wrap(&bob, &period).unwrap().data_hash, bob_hash);
}
