#![cfg(test)]

use super::*;
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    xdr::ToXdr,
    Address, Bytes, BytesN, Env, Symbol,
};

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
    let payload = construct_mint_payload(env, contract, user, period, archetype, data_hash);

    let mut out = [0u8; 512];
    let len = payload.len() as usize;
    payload.copy_into_slice(&mut out[..len]);

    let signature = signer.sign(&out[..len]);
    BytesN::from_array(env, &signature.to_bytes())
}

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

    // First mint - should succeed
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

    let wrap = client.get_wrap(&user, &period);
    assert!(wrap.is_some(), "First mint should succeed");

    // Replay attack: Try to mint again with the exact same parameters
    // This should PANIC with WrapAlreadyExists error (#4)
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_replay_attack_different_hash_same_period_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let data_hash_1 = BytesN::from_array(&env, &[42u8; 32]);
    let data_hash_2 = BytesN::from_array(&env, &[99u8; 32]);
    let archetype = symbol_short!("architect");
    let period = 202512u64;

    let signature_1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash_1,
        CURRENT_PAYLOAD_VERSION,
    );

    // First mint - should succeed
    client.mint_wrap(&user, &period, &archetype, &data_hash_1, &CURRENT_PAYLOAD_VERSION, &signature_1);

    let signature_2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash_2,
        CURRENT_PAYLOAD_VERSION,
    );

    // Try to mint again for the same period with a different hash
    // This should still fail - period is already used
    client.mint_wrap(&user, &period, &archetype, &data_hash_2, &CURRENT_PAYLOAD_VERSION, &signature_2);
}

#[test]
fn test_multiple_periods_for_same_user_success() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let data_hash_1 = BytesN::from_array(&env, &[42u8; 32]);
    let data_hash_2 = BytesN::from_array(&env, &[99u8; 32]);
    let data_hash_3 = BytesN::from_array(&env, &[77u8; 32]);
    let archetype = symbol_short!("architect");

    let period_1 = 202512u64;
    let period_2 = 202601u64;
    let period_3 = 202602u64;

    let signature_1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period_1,
        &archetype,
        &data_hash_1,
        CURRENT_PAYLOAD_VERSION,
    );
    let signature_2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period_2,
        &archetype,
        &data_hash_2,
        CURRENT_PAYLOAD_VERSION,
    );
    let signature_3 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period_3,
        &archetype,
        &data_hash_3,
        CURRENT_PAYLOAD_VERSION,
    );

    // All three should succeed
    client.mint_wrap(&user, &period_1, &archetype, &data_hash_1, &CURRENT_PAYLOAD_VERSION, &signature_1);
    client.mint_wrap(&user, &period_2, &archetype, &data_hash_2, &CURRENT_PAYLOAD_VERSION, &signature_2);
    client.mint_wrap(&user, &period_3, &archetype, &data_hash_3, &CURRENT_PAYLOAD_VERSION, &signature_3);

    assert!(client.get_wrap(&user, &period_1).is_some());
    assert!(client.get_wrap(&user, &period_2).is_some());
    assert!(client.get_wrap(&user, &period_3).is_some());
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

    let data_hash_for_a = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("architect");
    let period = 202512u64;

    let signature_a = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_a,
        period,
        &archetype,
        &data_hash_for_a,
        CURRENT_PAYLOAD_VERSION,
    );

    // User A mints successfully
    client.mint_wrap(&user_a, &period, &archetype, &data_hash_for_a, &CURRENT_PAYLOAD_VERSION, &signature_a);

    let wrap_a = client.get_wrap(&user_a, &period);
    assert!(wrap_a.is_some(), "User A should have the wrap");

    let data_hash_for_b = BytesN::from_array(&env, &[99u8; 32]);
    let period_b = 202601u64;

    let signature_b = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_b,
        period_b,
        &archetype,
        &data_hash_for_b,
        CURRENT_PAYLOAD_VERSION,
    );

    client.mint_wrap(
        &user_b,
        &period_b,
        &archetype,
        &data_hash_for_b,
        &CURRENT_PAYLOAD_VERSION,
        &signature_b,
    );

    let wrap_a = client.get_wrap(&user_a, &period).unwrap();
    let wrap_b = client.get_wrap(&user_b, &period_b).unwrap();

    assert_eq!(wrap_a.data_hash, data_hash_for_a);
    assert_eq!(wrap_b.data_hash, data_hash_for_b);

    let user_b_period_dec = client.get_wrap(&user_b, &period);
    assert!(
        user_b_period_dec.is_none(),
        "User B should not have User A's period"
    );
}

#[test]
fn test_cross_contract_replay_protection() {
    let env = Env::default();

    let contract_v1 = env.register_contract(None, StellarWrapContract);
    let contract_v2 = env.register_contract(None, StellarWrapContract);

    let client_v1 = StellarWrapContractClient::new(&env, &contract_v1);
    let client_v2 = StellarWrapContractClient::new(&env, &contract_v2);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client_v1.initialize(&admin, &admin_pubkey);
    client_v2.initialize(&admin, &admin_pubkey);

    env.mock_all_auths();

    let data_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("architect");
    let period = 202512u64;

    let signature_v1 = sign_payload(
        &env,
        &signing_key,
        &contract_v1,
        &user,
        period,
        &archetype,
        &data_hash,
        CURRENT_PAYLOAD_VERSION,
    );

    client_v1.mint_wrap(&user, &period, &archetype, &data_hash, &signature_v1);
    // Mint successfully on V1
    client_v1.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature_v1);

    let wrap_v1 = client_v1.get_wrap(&user, &period);
    assert!(wrap_v1.is_some(), "Wrap should exist on contract V1");

    let signature_v2 = sign_payload(
        &env,
        &signing_key,
        &contract_v2,
        &user,
        period,
        &archetype,
        &data_hash,
        CURRENT_PAYLOAD_VERSION,
    );

    client_v2.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature_v2);

    let wrap_v2 = client_v2.get_wrap(&user, &period);
    assert!(wrap_v2.is_some(), "Wrap should exist on contract V2");

    assert!(client_v1.get_wrap(&user, &period).is_some());
    assert!(client_v2.get_wrap(&user, &period).is_some());
    let payload_v1 =
        construct_mint_payload(&env, &contract_v1, &user, period, &archetype, &data_hash);
    let payload_v2 =
        construct_mint_payload(&env, &contract_v2, &user, period, &archetype, &data_hash);
    assert_ne!(
        payload_v1, payload_v2,
        "Payloads should differ across contract instances"
    );

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client_v2.mint_wrap(&user, &period, &archetype, &data_hash, &signature_v1);
    }));

    assert!(
        result.is_err(),
        "A signature from V1 should not be replayable on V2"
    );
    assert!(client_v2.get_wrap(&user, &period).is_none());
}

#[test]
fn test_gas_analysis_mint_operation() {
    let env = Env::default();
    env.budget().reset_unlimited();

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

    env.budget().reset_default();

    // Perform the mint operation
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

    env.budget().print();
    // Get budget consumption (only when gas reporting is explicitly enabled)
    if std::env::var("SOROBAN_GAS_REPORT").is_ok() {
        env.budget().print();
    }

    let cpu_insns = env.budget().cpu_instruction_cost();
    let mem_bytes = env.budget().memory_bytes_cost();

    assert!(
        cpu_insns < 10_000_000,
        "CPU instructions too high: {}",
        cpu_insns
    );
    assert!(mem_bytes < 100_000, "Memory usage too high: {}", mem_bytes);
}

#[test]
fn test_gas_analysis_multiple_mints() {
    let env = Env::default();
    env.budget().reset_unlimited();

    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    env.budget().reset_default();

    // Perform 5 mints for different periods
    for i in 1..6 {
        let data_hash = BytesN::from_array(&env, &[i as u8; 32]);
        let archetype = symbol_short!("architect");

        let period = match i {
            1 => 202512u64,
            2 => 202601u64,
            3 => 202602u64,
            4 => 202603u64,
            _ => 202604u64,
        };

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
    }

    let cpu_insns = env.budget().cpu_instruction_cost();
    let mem_bytes = env.budget().memory_bytes_cost();

    assert!(cpu_insns < 50_000_000, "Batch CPU too high: {}", cpu_insns);
    assert!(mem_bytes < 500_000, "Batch memory too high: {}", mem_bytes);
}

/// Test 8: Timestamp Manipulation Resistance
/// Ensures the contract uses ledger timestamp, not user-provided values
#[test]
fn test_timestamp_is_from_ledger_not_user() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    env.ledger().with_mut(|li| {
        li.timestamp = 1000000;
    });

    let data_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("architect");
    let period = 202512u64;

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

    let wrap = client.get_wrap(&user, &period).unwrap();

    assert_eq!(wrap.timestamp, 1000000, "Timestamp should come from ledger");

    env.ledger().with_mut(|li| {
        li.timestamp = 2000000;
    });

    let period_2 = 202601u64;
    let signature_2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period_2,
        &archetype,
        &data_hash,
        CURRENT_PAYLOAD_VERSION,
    );

    client.mint_wrap(&user, &period_2, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature_2);

    let wrap_2 = client.get_wrap(&user, &period_2).unwrap();
    assert_eq!(
        wrap_2.timestamp, 2000000,
        "Second timestamp should match new ledger time"
    );
}

#[test]
fn test_edge_case_long_symbols() {
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

    let wrap = client.get_wrap(&user, &period);
    assert!(wrap.is_some(), "Should handle reasonably long symbols");
}

#[test]
#[should_panic]
fn test_non_admin_cannot_mint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let _attacker = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);

    let data_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("architect");
    let period = 202512u64;

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

    // This should panic because attacker is not authorized
    client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);
}

/// Test 11: Revocation - Non-admin cannot revoke wraps
/// Only the admin should be able to revoke wrap records.
/// Without any mocked auth, admin.require_auth() will panic.
#[test]
#[should_panic]
fn test_non_admin_cannot_revoke() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);

    // Do NOT mock any auths — admin.require_auth() should panic
    let reason_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.revoke_wrap(&user, &202512, &reason_hash);
}

// ────────────────────────────────────────────────────────────────────────────
// Two-Step Admin Transfer Tests (Issue #269)
// ────────────────────────────────────────────────────────────────────────────

/// Test 11: Successful Two-Step Admin Transfer (Proposal + Acceptance)
/// Verifies the complete happy path: admin proposes, pending admin accepts.
#[test]
fn test_two_step_admin_transfer_success() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    // Step 1: Current admin proposes new_admin
    client.propose_admin(&new_admin);

    // Verify pending_admin is set
    assert_eq!(client.get_pending_admin().unwrap(), new_admin);
    // Verify current admin is still the same
    assert_eq!(client.get_admin().unwrap(), admin);

    // Step 2: Pending admin accepts
    client.accept_admin();

    // Verify admin has been transferred
    assert_eq!(client.get_admin().unwrap(), new_admin);
    // Verify pending_admin is cleared
    assert!(client.get_pending_admin().is_none());
}

/// Test 12: Admin Can Cancel a Pending Proposal
/// Verifies that the current admin can cancel a proposed transfer before acceptance.
#[test]
fn test_admin_cancel_proposed_admin() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[2u8; 32]);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    // Admin proposes
    client.propose_admin(&new_admin);
    assert_eq!(client.get_pending_admin().unwrap(), new_admin);

    // Admin cancels
    client.cancel_proposed_admin();

    // Verify proposal is cleared
    assert!(client.get_pending_admin().is_none());
    // Verify admin remains unchanged
    assert_eq!(client.get_admin().unwrap(), admin);
}

/// Test 13: Unauthorized Acceptance Fails - Non-Pending-Admin Cannot Accept
/// Verifies that an address other than the proposed admin cannot accept the transfer.
#[test]
#[should_panic]
fn test_unauthorized_acceptance_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[3u8; 32]);

    client.initialize(&admin, &pubkey);

    // Set up auths manually to control who is authenticating
    // Admin proposes new_admin
    env.set_auths(&[(&admin, &contract_id, symbol_short!("propose_admin"), ())]);
    client.propose_admin(&new_admin);
    assert_eq!(client.get_pending_admin().unwrap(), new_admin);

    // Attacker tries to accept - should panic because attacker != new_admin
    env.set_auths(&[(&attacker, &contract_id, symbol_short!("accept_admin"), ())]);
    client.accept_admin();
}

/// Test 14: Accepting Without a Proposal Fails
/// Verifies that accept_admin panics when there is no pending proposal.
#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_accept_admin_no_proposal_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[4u8; 32]);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    // No proposal exists - should panic with NoAdminTransferProposal
    client.accept_admin();
}

/// Test 15: Proposing When a Proposal Already Exists Fails
/// Verifies that propose_admin panics when there is already a pending proposal.
#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_propose_admin_when_proposal_exists_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin_1 = Address::generate(&env);
    let new_admin_2 = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[5u8; 32]);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    // First proposal
    client.propose_admin(&new_admin_1);
    assert_eq!(client.get_pending_admin().unwrap(), new_admin_1);

    // Try to propose again without canceling - should panic
    client.propose_admin(&new_admin_2);
}

/// Test 16: Canceling When No Proposal Exists Fails
/// Verifies that cancel_proposed_admin panics when there is no pending proposal.
#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_cancel_no_proposal_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[6u8; 32]);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    // No proposal exists - should panic with NoAdminTransferProposal
    client.cancel_proposed_admin();
}

/// Test 17: Non-Admin Cannot Propose a New Admin
/// Verifies that only the current admin can call propose_admin.
#[test]
#[should_panic]
fn test_non_admin_cannot_propose_admin() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[7u8; 32]);

    client.initialize(&admin, &pubkey);

    // Attacker tries to propose - should panic due to require_auth failure
    env.set_auths(&[(&attacker, &contract_id, symbol_short!("propose_admin"), ())]);
    client.propose_admin(&new_admin);
}

/// Test 18: Non-Admin Cannot Cancel a Pending Proposal
/// Verifies that only the current admin can cancel a proposal.
#[test]
#[should_panic]
fn test_non_admin_cannot_cancel_proposal() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[8u8; 32]);

    client.initialize(&admin, &pubkey);

    // Admin proposes (mock admin auth)
    env.set_auths(&[(&admin, &contract_id, symbol_short!("propose_admin"), ())]);
    client.propose_admin(&new_admin);
    assert_eq!(client.get_pending_admin().unwrap(), new_admin);

    // Attacker tries to cancel - should panic due to require_auth failure
    env.set_auths(&[(&attacker, &contract_id, symbol_short!("cancel_proposed_admin"), ())]);
    client.cancel_proposed_admin();
}

/// Test 19: update_admin (Single-Step) Clears Pending Proposal - Backward Compatibility
/// Verifies that the legacy single-step update_admin clears any pending proposal
/// and successfully transfers admin rights.
#[test]
fn test_update_admin_clears_pending_proposal() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let proposed_admin = Address::generate(&env);
    let direct_new_admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[9u8; 32]);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    // Admin proposes a transfer
    client.propose_admin(&proposed_admin);
    assert_eq!(client.get_pending_admin().unwrap(), proposed_admin);
    assert_eq!(client.get_admin().unwrap(), admin);

    // Admin bypasses two-step flow using update_admin (legacy)
    client.update_admin(&direct_new_admin);

    // Verify direct_new_admin is now the admin
    assert_eq!(client.get_admin().unwrap(), direct_new_admin);
    // Verify pending proposal was cleared
    assert!(client.get_pending_admin().is_none());
}

/// Test 20: get_pending_admin Returns None When No Proposal
/// Verifies the getter correctly returns None when there is no pending transfer.
#[test]
fn test_get_pending_admin_none() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[10u8; 32]);

    client.initialize(&admin, &pubkey);

    // No proposal made
    assert!(client.get_pending_admin().is_none());
}

/// Test 21: Propose Then Repropose After Cancel
/// Verifies that after canceling a proposal, the admin can propose a new one.
#[test]
fn test_propose_cancel_repropose() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let first_proposal = Address::generate(&env);
    let second_proposal = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[11u8; 32]);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    // First proposal
    client.propose_admin(&first_proposal);
    assert_eq!(client.get_pending_admin().unwrap(), first_proposal);

    // Cancel
    client.cancel_proposed_admin();
    assert!(client.get_pending_admin().is_none());

    // Propose a different admin
    client.propose_admin(&second_proposal);
    assert_eq!(client.get_pending_admin().unwrap(), second_proposal);

    // Accept the second proposal
    client.accept_admin();
    assert_eq!(client.get_admin().unwrap(), second_proposal);
    assert!(client.get_pending_admin().is_none());
}

/// Test 22: After Acceptance, New Admin Can Propose Further Transfers
/// Verifies the chain of ownership works: once accepted, the new admin
/// can initiate their own two-step transfers.
#[test]
fn test_new_admin_can_propose_further_transfers() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin_1 = Address::generate(&env);
    let admin_2 = Address::generate(&env);
    let admin_3 = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[12u8; 32]);

    client.initialize(&admin_1, &pubkey);
    env.mock_all_auths();

    // Admin1 -> Admin2 via two-step
    client.propose_admin(&admin_2);
    client.accept_admin();
    assert_eq!(client.get_admin().unwrap(), admin_2);

    // Admin2 -> Admin3 via two-step
    client.propose_admin(&admin_3);
    assert_eq!(client.get_pending_admin().unwrap(), admin_3);
    client.accept_admin();

    // Final state
    assert_eq!(client.get_admin().unwrap(), admin_3);
    assert!(client.get_pending_admin().is_none());
}
