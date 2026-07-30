import re

with open('src/test.rs', 'r') as f:
    content = f.read()

# Fix 1: test_get_latest_wrap_single_mint
content = re.sub(
    r'fn test_get_latest_wrap_single_mint\(\) \{.*?let sig = sign_payload\(\s*let sig_a = sign_payload\(',
    r'fn test_get_latest_wrap_single_mint() {\n    let env = Env::default();\n    let contract_id = env.register_contract(None, StellarWrapContract);\n    let client = StellarWrapContractClient::new(&env, &contract_id);\n    let signing_key = SigningKey::from_bytes(&[1u8; 32]);\n    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());\n    let admin = Address::generate(&env);\n    let user = Address::generate(&env);\n    client.initialize(&admin, &admin_pubkey);\n    env.mock_all_auths();\n    let archetype = symbol_short!("soroban");\n    let hash = BytesN::from_array(&env, &[0u8; 32]);\n\n    let sig_a = sign_payload(',
    content,
    flags=re.DOTALL
)

# Fix 2: test_migrate_rejects_replay
content = re.sub(
    r'fn test_migrate_rejects_replay\(\) \{.*?let signature = sign_payload\(\s*let sig1 = sign_payload\(',
    r'fn test_migrate_rejects_replay() {\n    let env = Env::default();\n    let contract_id = env.register_contract(None, StellarWrapContract);\n    let client = StellarWrapContractClient::new(&env, &contract_id);\n    let signing_key = SigningKey::from_bytes(&[1u8; 32]);\n    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());\n    let admin = Address::generate(&env);\n    let user = Address::generate(&env);\n    client.initialize(&admin, &admin_pubkey);\n    env.mock_all_auths();\n    let archetype = symbol_short!("soroban");\n    let hash = BytesN::from_array(&env, &[0u8; 32]);\n\n    let sig1 = sign_payload(',
    content,
    flags=re.DOTALL
)

# Fix 3: test_fsm_valid_state_transitions
content = re.sub(
    r'fn test_fsm_valid_state_transitions\(\) \{.*?let sig = sign_payload\(\s*let sig_old = sign_payload\(',
    r'fn test_fsm_valid_state_transitions() {\n    let env = Env::default();\n    let contract_id = env.register_contract(None, StellarWrapContract);\n    let client = StellarWrapContractClient::new(&env, &contract_id);\n    let signing_key = SigningKey::from_bytes(&[1u8; 32]);\n    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());\n    let admin = Address::generate(&env);\n    let user = Address::generate(&env);\n    client.initialize(&admin, &admin_pubkey);\n    env.mock_all_auths();\n    let archetype = symbol_short!("soroban");\n    let hash = BytesN::from_array(&env, &[0u8; 32]);\n    let period_old = 202401u64;\n    let old_signing_key = signing_key;\n\n    let sig_old = sign_payload(',
    content,
    flags=re.DOTALL
)

# Fix 4: test_fsm_invalid_state_transition_fails
content = re.sub(
    r'fn test_fsm_invalid_state_transition_fails\(\) \{\s*&data_hash,\s*\);',
    r'fn test_fsm_invalid_state_transition_fails() {',
    content,
    flags=re.MULTILINE
)

# Fix 5: test_update_wrap_zero_hash_rejected
content = re.sub(
    r'fn test_update_wrap_zero_hash_rejected\(\) \{\s*&hash,\s*\);',
    r'fn test_update_wrap_zero_hash_rejected() {',
    content,
    flags=re.MULTILINE
)

# Fix 6: test_mint_guard_on_failure_leaves_no_residual_state
content = re.sub(
    r'fn test_mint_guard_on_failure_leaves_no_residual_state\(\) \{\s*&hash,\s*\);',
    r'fn test_mint_guard_on_failure_leaves_no_residual_state() {',
    content,
    flags=re.MULTILINE
)

# Fix 7: test_migrate_applies_once_per_version
content = re.sub(
    r'fn test_migrate_applies_once_per_version\(\) \{\s*fn test_get_mint_timestamp_exists\(\) \{',
    r'fn test_migrate_applies_once_per_version() {\n}\nfn test_get_mint_timestamp_exists() {',
    content,
    flags=re.MULTILINE
)

# Fix 8: test_get_wrap_returns_none_before_initialization
content = re.sub(
    r'fn test_get_wrap_returns_none_before_initialization\(\) \{\s*fn test_instance_ttl_extended_on_mint\(\) \{',
    r'fn test_get_wrap_returns_none_before_initialization() {\n}\nfn test_instance_ttl_extended_on_mint() {',
    content,
    flags=re.MULTILINE
)

with open('src/test.rs', 'w') as f:
    f.write(content)
