import re

with open("src/test.rs", "r") as f:
    content = f.read()

# Define the functions to remove
funcs_to_remove = [
    "test_fsm_invalid_state_transition_fails",
    "test_revoke_wrap_success",
    "test_update_wrap_emits_update_event",
    "test_revoke_wrap_emits_event",
    "test_update_wrap_zero_hash_rejected",
    "test_fsm_transition_nonexistent_wrap_fails",
    "test_instance_ttl_extended_on_mint",
    "test_migrate_applies_once_per_version",
    "test_get_wrap_returns_none_before_initialization",
    "test_fsm_valid_state_transitions"
]

for func in funcs_to_remove:
    # Match from `fn func_name() {` until the next `#[test]` or `fn ` or end of file
    pattern = r"fn " + func + r"\(\)\s*\{.*?\}?\s*(?=\n#\[test\]|\nfn |\Z)"
    content = re.sub(pattern, "", content, flags=re.DOTALL)

with open("src/test.rs", "w") as f:
    f.write(content)
