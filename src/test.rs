#![cfg(test)]

extern crate std;

use super::*;
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

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &dummy_hash,
    );
    client.mint_wrap(&user, &period, &archetype, &dummy_hash, &signature);

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

    client.mint_wrap(&user, &period, &archetype, &hash, &signature);

    let events = env.events().all();
    let last_event = events.last().expect("no events found");
    let (_, topics, data) = last_event;

    let event_topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let event_user: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let event_period: u64 = topics.get(2).unwrap().try_into_val(&env).unwrap();
    let event_archetype: Symbol = data.try_into_val(&env).unwrap();

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
    );
    let sig_b = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_b,
        period_b,
        &archetype_b,
        &hash,
    );

    client.mint_wrap(&user_a, &period_a, &archetype_a, &hash, &sig_a);
    client.mint_wrap(&user_b, &period_b, &archetype_b, &hash, &sig_b);
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
        &hash,
    );
    client.mint_wrap(&user, &202401, &archetype, &hash, &sig1);

    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202402,
        &archetype,
        &hash,
    );
    client.mint_wrap(&user, &202402, &archetype, &hash, &sig2);

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

    client.mint_wrap(&user, &period, &archetype, &hash, &sig);
    client.mint_wrap(&user, &period, &archetype, &hash, &sig);
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

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
    );
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

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
    );
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
    );
    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202404,
        &archetype,
        &hash2,
    );
    let sig3 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        202403,
        &archetype,
        &hash3,
    );

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

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &sig);

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
    );
    let upper_sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        210012,
        &archetype,
        &upper_hash,
    );

    client.mint_wrap(&user, &202401, &archetype, &lower_hash, &lower_sig);
    client.mint_wrap(&user, &210012, &archetype, &upper_hash, &upper_sig);

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
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
    );

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
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
    );

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
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
    );

    client.mint_wrap(&user, &period, &archetype, &hash, &signature);
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
        );

        client.mint_wrap(&user, &period, &archetype, &hash, &signature);

        cpu_samples[i] = env.budget().cpu_instruction_cost();
        mem_samples[i] = env.budget().memory_bytes_cost();
        users.push(user);
    }

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
    client.mint_wrap(&user, &period, &archetype, &dummy_hash, &signature);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(
        client.get_mint_timestamp(&user, &period),
        Some(wrap.timestamp)
    );
}

#[test]
fn test_get_mint_timestamp_missing() {
    let period = 202406u64;
    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[14u8; 32]);

    let sig = sign_payload(&env, &signing_key, &contract_id, &user, period, &archetype, &hash);
    client.mint_wrap(&user, &period, &archetype, &hash, &sig);

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
    client.mint_wrap(&user, &period_old, &archetype, &hash, &sig_old);

    // Rotate to new key
    let new_signing_key = SigningKey::from_bytes(&[98u8; 32]);
    let new_pubkey = BytesN::from_array(&env, &new_signing_key.verifying_key().to_bytes());
    client.update_admin_pubkey(&new_pubkey);

    // Mint with new key — should succeed
    let period_new = 202409u64;
    let sig_new = sign_payload(&env, &new_signing_key, &contract_id, &user, period_new, &archetype, &hash);
    client.mint_wrap(&user, &period_new, &archetype, &hash, &sig_new);

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

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &dummy_hash,
    );
    client.mint_wrap(&user, &period, &archetype, &dummy_hash, &signature);

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
    client.mint_wrap(&user, &period, &archetype, &dummy_hash, &signature);

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
    client.mint_wrap(&user, &period, &archetype, &hash, &sig_old);
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn test_update_admin_pubkey_requires_admin_auth() {
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
    client.mint_wrap(&user_a, &202407u64, &archetype, &hash, &sig_a);

    let sig_b = sign_payload(&env, &signing_key, &contract_id, &user_b, 202407u64, &archetype, &hash);
    client.mint_wrap(&user_b, &202407u64, &archetype, &hash, &sig_b);

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
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_update_admin_pubkey_before_init_fails() {
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

    // Mint first wrap
    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, 2024, &archetype, &hash);
    client.mint_wrap(&user, &2024, &archetype, &hash, &sig1);

    // After first mint: balance = 1, latest = 2024
    assert_eq!(client.balance_of(&user), 1);
    assert_eq!(client.get_latest_wrap(&user).unwrap().period, 2024);

    // Mint second wrap (higher period)
    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 2025, &archetype, &hash);
    client.mint_wrap(&user, &2025, &archetype, &hash, &sig2);

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

    // Mint first wrap (period 2024)
    let sig1 = sign_payload(&env, &signing_key, &contract_id, &user, 2024, &archetype, &hash1);
    client.mint_wrap(&user, &2024, &archetype, &hash1, &sig1);

    // Mint second wrap (period 2025)
    let sig2 = sign_payload(&env, &signing_key, &contract_id, &user, 2025, &archetype, &hash2);
    client.mint_wrap(&user, &2025, &archetype, &hash2, &sig2);

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

    let signing_key = SigningKey::from_bytes(&[52u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let archetype = symbol_short!("arch");

    let sig = sign_payload(&env, &signing_key, &contract_id, &user, 2024, &archetype, &hash);
    client.mint_wrap(&user, &2024, &archetype, &hash, &sig);

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

    let signing_key = SigningKey::from_bytes(&[53u8; 32]);
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
    let (_, topics, data) = last_event;

    let t0: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let t1: Symbol = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let emitted_key: BytesN<32> = data.try_into_val(&env).unwrap();

    assert_eq!(t0, symbol_short!("admin"));
    assert_eq!(t1, symbol_short!("pubkey"));
    assert_eq!(emitted_key, new_pubkey);
}
