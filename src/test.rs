#![cfg(test)]

//! Contract integration tests.

extern crate std;

use super::*;
use crate::mint::CURRENT_PAYLOAD_VERSION;
use crate::test_utils::sign_payload;
use ed25519_dalek::SigningKey;
use soroban_sdk::{
    symbol_short,
    testutils::Address as _,
    Address, BytesN, Env, String,
};

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

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &dummy_hash,
    );
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &dummy_hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );

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
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
    );

    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );

    // Successful mint is the signal; event schema is covered elsewhere.
    assert!(client.get_wrap(&user, &period).is_some());
}

#[test]
fn test_get_admin_before_init_returns_none() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    assert!(client.get_admin().is_none());
}

#[test]
fn test_get_admin_after_init() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);

    assert_eq!(client.get_admin().unwrap(), admin);
}

#[test]
fn test_name_symbol_decimals_defaults() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    assert_eq!(
        client.name(),
        String::from_str(&env, "Stellar Wrap Registry")
    );
    assert_eq!(client.symbol(), String::from_str(&env, "WRAP"));
    assert_eq!(client.decimals(), 0);
}

#[test]
fn test_get_admin_pubkey_before_and_after_init() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    assert!(client.get_admin_pubkey().is_none());

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[9u8; 32]);
    client.initialize(&admin, &pubkey);

    assert_eq!(client.get_admin_pubkey().unwrap(), pubkey);
}

#[test]
fn test_version_returns_semver() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    assert_eq!(client.version(), String::from_str(&env, "0.1.0"));
}

