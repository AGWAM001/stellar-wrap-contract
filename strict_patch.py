import re

with open('src/test.rs', 'r') as f:
    text = f.read()

text = text.replace("use soroban_sdk::{", "use crate::mint::CURRENT_PAYLOAD_VERSION;\nuse soroban_sdk::{")

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

# Parse balanced parentheses
def find_matching_paren(s, start):
    count = 0
    for i in range(start, len(s)):
        if s[i] == '(': count += 1
        elif s[i] == ')':
            count -= 1
            if count == 0:
                return i
    return -1

# Patch sign_payload(...)
def patch_sign_payload(t):
    pos = 0
    while True:
        idx = t.find("sign_payload(", pos)
        if idx == -1: break
        
        # Check if it's the definition
        if "fn sign_payload(" in t[max(0, idx-10):idx+14]:
            pos = idx + 10
            continue
            
        end_idx = find_matching_paren(t, idx + 12)
        if end_idx != -1:
            # We found the call!
            # Insert the argument right before the closing parenthesis.
            # Handle trailing commas.
            inner = t[idx+13:end_idx]
            if inner.strip().endswith(','):
                t = t[:end_idx] + " CURRENT_PAYLOAD_VERSION" + t[end_idx:]
            else:
                t = t[:end_idx] + ", CURRENT_PAYLOAD_VERSION" + t[end_idx:]
            pos = end_idx + 25 # skip past
        else:
            pos = idx + 10
    return t

text = patch_sign_payload(text)

# Patch client.mint_wrap(...)
def patch_mint_wrap(t):
    pos = 0
    while True:
        idx = t.find("client.mint_wrap(", pos)
        if idx == -1: break
        end_idx = find_matching_paren(t, idx + 16)
        if end_idx != -1:
            # mint_wrap has 5 args, we need to insert payload_version as 5th (index 4)
            # Just do it by splitting by commas on the top level.
            inner = t[idx+17:end_idx]
            
            # Simple top-level comma split
            args = []
            cur = ""
            depth = 0
            for c in inner:
                if c == '(': depth += 1
                elif c == ')': depth -= 1
                
                if c == ',' and depth == 0:
                    args.append(cur)
                    cur = ""
                else:
                    cur += c
            args.append(cur)
            
            if len(args) == 5:
                # Add payload_version before the last argument
                args.insert(4, " &CURRENT_PAYLOAD_VERSION")
                new_inner = ",".join(args)
                t = t[:idx+17] + new_inner + t[end_idx:]
                pos = idx + 17 + len(new_inner) + 1
            else:
                pos = end_idx + 1
        else:
            pos = idx + 10
    return t

text = patch_mint_wrap(text)

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

with open('src/test.rs', 'w') as f:
    f.write(text)
