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

    // First mint, user A.
    let user_a = Address::generate(&env);
    let sig_a = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_a,
        202401,
        &archetype,
        &hash,
    );
    client.mint_wrap(&user_a, &202401, &archetype, &hash, &sig_a);
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
        &hash,
    );
    client.mint_wrap(&user_a, &202402, &archetype, &hash, &sig_a2);
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
        &hash,
    );
    client.mint_wrap(&user_b, &202401, &archetype, &hash, &sig_b);
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