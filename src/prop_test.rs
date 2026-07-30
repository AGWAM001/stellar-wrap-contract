//! Property-based tests for the StellarWrap contract.
#![cfg(test)]
extern crate std;

use super::*;
use crate::mint::CURRENT_PAYLOAD_VERSION;
use crate::test_utils::sign_payload;
use ed25519_dalek::SigningKey;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Symbol};

#[test]
fn test_prop_mint_and_get() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let signing_key = SigningKey::from_bytes(&[0xABu8; 32]);
    let pubkey_bytes = signing_key.verifying_key().to_bytes();
    let admin_pubkey = BytesN::from_array(&env, &pubkey_bytes);
    let admin = Address::generate(&env);
    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();
    let user = Address::generate(&env);
    let archetype = Symbol::new(&env, "arch");
    let data_hash = BytesN::from_array(&env, &[1u8; 32]);
    let period = 202401u64;
    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &data_hash);
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &sig);
    assert!(client.get_wrap(&user, &period).is_some());
}