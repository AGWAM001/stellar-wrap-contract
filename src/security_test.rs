#![cfg(test)]

//! Adversarial tests for mint signature verification and replay protection.

use super::*;
use crate::mint::CURRENT_PAYLOAD_VERSION;
use crate::signature::construct_mint_payload;
use crate::test_utils::sign_payload;
use ed25519_dalek::SigningKey;
use soroban_sdk::{
    symbol_short,
    testutils::Address as _,
    Address, BytesN, Env,
};

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

    let period = 202401u64;
    let archetype = symbol_short!("arch");
    let data_hash = BytesN::from_array(&env, &[42u8; 32]);
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
    );

    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &data_hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );
    client.mint_wrap(
        &user,
        &period,
        &archetype,
        &data_hash,
        &CURRENT_PAYLOAD_VERSION,
        &signature,
    );
}

#[test]
fn test_payload_bound_to_contract_id() {
    let env = Env::default();
    let contract_v1 = env.register_contract(None, StellarWrapContract);
    let contract_v2 = env.register_contract(None, StellarWrapContract);
    let user = Address::generate(&env);
    let archetype = symbol_short!("arch");
    let data_hash = BytesN::from_array(&env, &[7u8; 32]);
    let period = 202402u64;

    let p1 = construct_mint_payload(
        &env,
        &contract_v1,
        &user,
        period,
        &archetype,
        &data_hash,
        CURRENT_PAYLOAD_VERSION,
    );
    let p2 = construct_mint_payload(
        &env,
        &contract_v2,
        &user,
        period,
        &archetype,
        &data_hash,
        CURRENT_PAYLOAD_VERSION,
    );
    assert_ne!(p1, p2);
}
