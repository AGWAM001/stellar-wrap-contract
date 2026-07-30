import re

with open('src/test.rs', 'r') as f:
    text = f.read()

# Add CURRENT_PAYLOAD_VERSION
text = text.replace("use soroban_sdk::{", "use crate::mint::CURRENT_PAYLOAD_VERSION;\nuse soroban_sdk::{")

# Fix sign_payload definition
text = text.replace(
"""fn sign_payload(
    env: &Env,
    signer: &SigningKey,
    contract: &Address,
    user: &Address,
    period: u64,
    archetype: &Symbol,
    data_hash: &BytesN<32>,
) -> BytesN<64> {""",
"""fn sign_payload(
    env: &Env,
    signer: &SigningKey,
    contract: &Address,
    user: &Address,
    period: u64,
    archetype: &Symbol,
    data_hash: &BytesN<32>,
    payload_version: u32,
) -> BytesN<64> {""")

text = text.replace(
"""    payload.append(&contract.to_xdr(env));""",
"""    payload.append(&payload_version.to_xdr(env));
    payload.append(&contract.to_xdr(env));""")

# Fix all sign_payload calls. They are all formatted with trailing comma or just closing parenthesis.
text = re.sub(
    r'(sign_payload\([^)]+?)\s*\)',
    r'\1, CURRENT_PAYLOAD_VERSION)',
    text
)
# Re-fix the definition because the above regex hits the definition too!
text = text.replace(
"""fn sign_payload(
    env: &Env,
    signer: &SigningKey,
    contract: &Address,
    user: &Address,
    period: u64,
    archetype: &Symbol,
    data_hash: &BytesN<32>,
    payload_version: u32,
    CURRENT_PAYLOAD_VERSION) -> BytesN<64> {""",
"""fn sign_payload(
    env: &Env,
    signer: &SigningKey,
    contract: &Address,
    user: &Address,
    period: u64,
    archetype: &Symbol,
    data_hash: &BytesN<32>,
    payload_version: u32,
) -> BytesN<64> {""")

# Fix client.mint_wrap calls
def repl_mint(m):
    args = m.group(1).split(',')
    if len(args) == 5:
        return f"client.mint_wrap({args[0]},{args[1]},{args[2]},{args[3]}, &CURRENT_PAYLOAD_VERSION,{args[4]})"
    return m.group(0)

text = re.sub(r'client\.mint_wrap\(([^)]+)\)', repl_mint, text)

# Fix SDK 27 events
text = text.replace(
    'let last_event = events.last().expect("no events found");',
    'let last_event = events.events().last().expect("no events found");'
)
text = text.replace(
    'let (_, topics, data) = last_event;',
    'let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;\n    let topics = &v0.topics;\n    let data = &v0.data;'
)
text = text.replace(
    'let event_period: u64 = topics.get(2).unwrap().try_into_val(&env).unwrap();',
    'let event_period: u64 = soroban_sdk::Val::try_from_val(&env, topics.get(2).unwrap()).unwrap().try_into_val(&env).unwrap();'
)
text = text.replace(
    'let event_topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();',
    'let event_topic: Symbol = soroban_sdk::Val::try_from_val(&env, topics.get(0).unwrap()).unwrap().try_into_val(&env).unwrap();'
)
text = text.replace(
    'let event_user: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();',
    'let event_user: Address = soroban_sdk::Val::try_from_val(&env, topics.get(1).unwrap()).unwrap().try_into_val(&env).unwrap();'
)

with open('src/test.rs', 'w') as f:
    f.write(text)
