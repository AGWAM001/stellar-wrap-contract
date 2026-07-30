import re

with open('src/test.rs', 'r') as f:
    content = f.read()

# Add use crate::mint::CURRENT_PAYLOAD_VERSION;
if "CURRENT_PAYLOAD_VERSION" not in content:
    content = content.replace("use soroban_sdk::{", "use crate::mint::CURRENT_PAYLOAD_VERSION;\nuse soroban_sdk::{")

# Update sign_payload signature
content = re.sub(
    r'fn sign_payload\(\n    env: &Env,\n    signer: &SigningKey,\n    contract: &Address,\n    user: &Address,\n    period: u64,\n    archetype: &Symbol,\n    data_hash: &BytesN<32>,\n\) -> BytesN<64> \{',
    r'fn sign_payload(\n    env: &Env,\n    signer: &SigningKey,\n    contract: &Address,\n    user: &Address,\n    period: u64,\n    archetype: &Symbol,\n    data_hash: &BytesN<32>,\n    payload_version: u32,\n) -> BytesN<64> {',
    content,
    flags=re.MULTILINE
)

# Update sign_payload body
content = re.sub(
    r'payload\.append\(&contract\.to_xdr\(env\)\);',
    r'payload.append(&payload_version.to_xdr(env));\n    payload.append(&contract.to_xdr(env));',
    content,
    flags=re.MULTILINE
)

# Replace all sign_payload(...) calls
def repl_sign(m):
    args = m.group(1).rstrip()
    if args.endswith(','):
        return f"sign_payload({args}\n        CURRENT_PAYLOAD_VERSION,"
    else:
        return f"sign_payload({args}, CURRENT_PAYLOAD_VERSION"

content = re.sub(r'sign_payload\((.*?)\)', repl_sign, content, flags=re.DOTALL)
# The above regex will hit the definition too. Let's fix the definition back:
content = content.replace(
    'fn sign_payload(\n    env: &Env,\n    signer: &SigningKey,\n    contract: &Address,\n    user: &Address,\n    period: u64,\n    archetype: &Symbol,\n    data_hash: &BytesN<32>,\n    payload_version: u32,\n        CURRENT_PAYLOAD_VERSION, -> BytesN<64> {',
    'fn sign_payload(\n    env: &Env,\n    signer: &SigningKey,\n    contract: &Address,\n    user: &Address,\n    period: u64,\n    archetype: &Symbol,\n    data_hash: &BytesN<32>,\n    payload_version: u32,\n) -> BytesN<64> {'
)

# Update client.mint_wrap(...)
def repl_mint(m):
    # args match: user, period, archetype, hash, signature
    args = m.group(1)
    parts = args.split(',')
    if len(parts) == 5:
        return f"client.mint_wrap({parts[0]},{parts[1]},{parts[2]},{parts[3]}, &CURRENT_PAYLOAD_VERSION,{parts[4]})"
    return f"client.mint_wrap({args})"

content = re.sub(r'client\.mint_wrap\((.*?)\)', repl_mint, content, flags=re.DOTALL)

# Also fix the SDK 27 events issue in test_mint_emits_event
content = content.replace(
    'let last_event = events.last().expect("no events found");\n    let (_, topics, data) = last_event;',
    'let last_event = events.events().last().expect("no events found");\n    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last_event.body;\n    let topics = &v0.topics;\n    let data = &v0.data;'
)
content = content.replace(
    'let event_period: u64 = topics.get(2).unwrap().try_into_val(&env).unwrap();',
    'let event_period: u64 = soroban_sdk::Val::try_from_val(&env, topics.get(2).unwrap()).unwrap().try_into_val(&env).unwrap();'
)

with open('src/test.rs', 'w') as f:
    f.write(content)
