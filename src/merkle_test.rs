#![cfg(test)]

use ed25519_dalek::SigningKey;
use rand::thread_rng;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, BytesN as _, Events},
    vec, Address, BytesN, Env, IntoVal, Symbol, Val, Vec,
};

use crate::{
    merkle::{compute_whitelist_leaf, hash_pair},
    StellarWrapContract, StellarWrapContractClient,
    ContractError,
};
use crate::test_utils::decode_events;

fn setup() -> (Env, StellarWrapContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let mut csprng = thread_rng();
    let signer = SigningKey::generate(&mut csprng);
    let admin_pubkey = BytesN::from_array(&env, &signer.verifying_key().to_bytes());

    client.initialize(&admin, &admin_pubkey);
    (env, client, admin)
}

#[test]
fn test_merkle_whitelist() {
    let (env, client, admin) = setup();

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);
    let non_member = Address::generate(&env);

    let empty_proof = vec![&env];

    // 1. verify_whitelist before any root is published fails with MerkleRootNotSet
    let res = client.try_verify_whitelist(&user1, &empty_proof);
    assert_eq!(res, Err(Ok(ContractError::MerkleRootNotSet.into())));

    // Build tree: [user1, user2, user3]
    let leaf1 = client.whitelist_leaf(&user1);
    let leaf2 = client.whitelist_leaf(&user2);
    let leaf3 = client.whitelist_leaf(&user3);

    // root
    // ├── node12
    // │   ├── leaf1
    // │   └── leaf2
    // └── leaf3
    let node12 = hash_pair(&env, &leaf1, &leaf2);
    let root = hash_pair(&env, &node12, &leaf3);

    // 7. set_whitelist_root requires admin authorization and emits ("whitelist", "root") event.
    client.set_whitelist_root(&root);
    
    // Verify admin auth was checked (mock_all_auths makes this pass, but we can check if it's there conceptually.
    // In Soroban tests with mock_all_auths, auths are tracked.)
    assert_eq!(
        env.auths().first().unwrap().0,
        admin
    );

    let events = decode_events(&env);
    let last_event = events.last().unwrap();
    let topics = &last_event.0;
    assert_eq!(topics.len(), 2);
    assert_eq!(topics[0], symbol_short!("whitelist").into_val(&env));
    assert_eq!(topics[1], symbol_short!("root").into_val(&env));
    assert_eq!(last_event.1, root.into_val(&env));

    // 2. A valid proof for a member returns true.
    let proof1 = vec![&env, leaf2.clone(), leaf3.clone()];
    assert!(client.verify_whitelist(&user1, &proof1));

    let proof2 = vec![&env, leaf1.clone(), leaf3.clone()];
    assert!(client.verify_whitelist(&user2, &proof2));

    let proof3 = vec![&env, node12.clone()];
    assert!(client.verify_whitelist(&user3, &proof3));

    // 3. A proof for a non-member returns false.
    let fake_proof = vec![&env, node12.clone()];
    assert!(!client.verify_whitelist(&non_member, &fake_proof));

    // 4. A truncated or reordered proof returns false.
    let truncated_proof = vec![&env, leaf2.clone()];
    assert!(!client.verify_whitelist(&user1, &truncated_proof));

    let reordered_proof = vec![&env, leaf3.clone(), leaf2.clone()];
    assert!(!client.verify_whitelist(&user1, &reordered_proof));

    // 6. clear_whitelist_root restores the MerkleRootNotSet behaviour.
    client.clear_whitelist_root();
    
    let res_cleared = client.try_verify_whitelist(&user1, &proof1);
    assert_eq!(res_cleared, Err(Ok(ContractError::MerkleRootNotSet.into())));
}

#[test]
fn test_single_leaf_tree() {
    let (env, client, _) = setup();
    let user = Address::generate(&env);

    let leaf = client.whitelist_leaf(&user);
    // Tree with one member: root == leaf
    client.set_whitelist_root(&leaf);

    // 5. An empty proof returns true only for a single-leaf tree where leaf == root.
    let empty_proof = vec![&env];
    assert!(client.verify_whitelist(&user, &empty_proof));

    // Empty proof for another user should be false (since their leaf != root)
    let other_user = Address::generate(&env);
    assert!(!client.verify_whitelist(&other_user, &empty_proof));
}
