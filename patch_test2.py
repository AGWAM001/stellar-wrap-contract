import re

with open('src/test.rs', 'r') as f:
    content = f.read()

if "CURRENT_PAYLOAD_VERSION" not in content:
    content = content.replace("use soroban_sdk::{", "use crate::mint::CURRENT_PAYLOAD_VERSION;\nuse soroban_sdk::{")

content = re.sub(
    r'fn sign_payload\(\n    env: &Env,\n    signer: &SigningKey,\n    contract: &Address,\n    user: &Address,\n    period: u64,\n    archetype: &Symbol,\n    data_hash: &BytesN<32>,\n\) -> BytesN<64> \{',
    r'fn sign_payload(\n    env: &Env,\n    signer: &SigningKey,\n    contract: &Address,\n    user: &Address,\n    period: u64,\n    archetype: &Symbol,\n    data_hash: &BytesN<32>,\n    payload_version: u32,\n) -> BytesN<64> {',
    content,
    flags=re.MULTILINE
)

content = re.sub(
    r'payload\.append\(&contract\.to_xdr\(env\)\);',
    r'payload.append(&payload_version.to_xdr(env));\n    payload.append(&contract.to_xdr(env));',
    content,
    flags=re.MULTILINE
)

# A robust way to append an argument to a function call in Rust is to just replace the specific sign_payload signature
# All sign_payload calls have 7 arguments.
# Let's replace the last argument passing and the parenthesis
content = re.sub(
    r'(sign_payload\([^)]+?)\s*\)',
    r'\1, CURRENT_PAYLOAD_VERSION)',
    content
)
# The above regex will match sign_payload(...) where there are no inner parentheses.
# Let's fix the definition again just in case it matched it
content = content.replace(
    'fn sign_payload(\n    env: &Env,\n    signer: &SigningKey,\n    contract: &Address,\n    user: &Address,\n    period: u64,\n    archetype: &Symbol,\n    data_hash: &BytesN<32>,\n    payload_version: u32,\n    CURRENT_PAYLOAD_VERSION) -> BytesN<64> {',
    'fn sign_payload(\n    env: &Env,\n    signer: &SigningKey,\n    contract: &Address,\n    user: &Address,\n    period: u64,\n    archetype: &Symbol,\n    data_hash: &BytesN<32>,\n    payload_version: u32,\n) -> BytesN<64> {'
)

# client.mint_wrap(...)
def repl_mint(m):
    args = m.group(1)
    parts = args.split(',')
    if len(parts) == 5:
        return f"client.mint_wrap({parts[0]},{parts[1]},{parts[2]},{parts[3]}, &CURRENT_PAYLOAD_VERSION,{parts[4]})"
    return f"client.mint_wrap({args})"

content = re.sub(r'client\.mint_wrap\(([^)]+)\)', repl_mint, content)

content = content.replace(
    'let last_event = events.last().expect("no events found");\n    let (_, topics, data) = last_event;',
    'let last_event = events.events().last().expect("no events found");\n    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;\n    let topics = &v0.topics;\n    let data = &v0.data;'
)
content = content.replace(
    'let event_period: u64 = topics.get(2).unwrap().try_into_val(&env).unwrap();',
    'let event_period: u64 = soroban_sdk::Val::try_from_val(&env, topics.get(2).unwrap()).unwrap().try_into_val(&env).unwrap();'
)
# wait, there's another events change needed for SDK 27?
# test_revoke_emits_event_multi_user has events.last() too.
content = content.replace(
    'let last_event = events.last().expect("no events found");',
    'let last_event = events.events().last().expect("no events found");'
)
content = content.replace(
    'let (_, topics, data) = last_event;',
    'let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;\n    let topics = &v0.topics;\n    let data = &v0.data;'
)
content = content.replace(
    'let event_topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();',
    'let event_topic: Symbol = soroban_sdk::Val::try_from_val(&env, topics.get(0).unwrap()).unwrap().try_into_val(&env).unwrap();'
)
content = content.replace(
    'let event_user: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();',
    'let event_user: Address = soroban_sdk::Val::try_from_val(&env, topics.get(1).unwrap()).unwrap().try_into_val(&env).unwrap();'
)

with open('src/test.rs', 'w') as f:
    f.write(content)
