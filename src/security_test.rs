#![cfg(test)]

use super::*;
use crate::mint::CURRENT_PAYLOAD_VERSION;
use crate::test_utils::sign_payload;
use ed25519_dalek::SigningKey;
use soroban_sdk::{symbol_short, testutils::{Address as _, Ledger}, Address, BytesN, Env};

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_replay_attack_same_period_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let data_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("architect");
    let period = 202512u64;
    let signature = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &data_hash);
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);
}

#[test]
fn test_signature_cannot_be_stolen_by_another_user() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let data_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202401u64;
    let sig_for_a = sign_payload(&env, &signing_key, &contract_id, &user_a, period, &archetype, &data_hash);
    let result = client.try_mint_wrap(&user_b, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &sig_for_a);
    assert!(result.is_err());
}