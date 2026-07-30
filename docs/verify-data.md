# Using `verify_data` for Off-Chain JSON Integrity Checks

`verify_data` lets you confirm that raw data bytes match the SHA-256 hash
committed to the chain at mint time. This guide explains exactly how to prepare
the input, what can go wrong, and shows matching and non-matching examples.

---

## How it works

When a wrap is minted the caller supplies a `data_hash: BytesN<32>`, which is
the SHA-256 digest of the raw data bytes they want to commit. The contract
stores that hash inside the `WrapRecord`.

`verify_data` accepts the original raw bytes and recomputes the hash on-chain:

```
stored_hash = WrapRecord.data_hash          (set at mint time)
computed    = sha256(data)                  (computed from the bytes you pass in)
result      = stored_hash == computed
```

It returns `true` only when the bytes you supply hash to exactly the value
stored in the record. If no wrap exists for the given `(user, period)` it
returns `false` without error.

---

## Preparing the input bytes

Pass the **exact byte sequence** that was hashed before minting — no
transformation, no re-encoding.

For JSON payloads this typically means:

1. Serialise your object to a UTF-8 JSON string.
2. Take the raw UTF-8 bytes.
3. Compute `sha256(bytes)` to get `data_hash` for `mint_wrap`.
4. Pass the same raw bytes to `verify_data` when you want to verify.

> **⚠ Whitespace and field ordering are significant.**
>
> SHA-256 is a byte-for-byte hash. Any difference between the bytes used at
> mint time and the bytes passed to `verify_data` will produce a different
> digest and the call will return `false`. Common pitfalls:
>
> - `{"score":100}` and `{ "score": 100 }` are different byte sequences.
> - `{"a":1,"b":2}` and `{"b":2,"a":1}` are different byte sequences.
> - A trailing newline (`\n`) changes the hash.
> - A BOM (byte-order mark) at the start of a file changes the hash.
>
> Canonical serialisation — a deterministic, whitespace-free format where keys
> are always in the same order — is strongly recommended so that any system
> can reproduce the same bytes independently. JSON Canonicalization Scheme
> ([RFC 8785](https://www.rfc-editor.org/rfc/rfc8785)) is one well-specified
> option.

---

## CLI usage

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- \
  verify_data \
  --user <USER_ADDRESS> \
  --period <PERIOD> \
  --data <DATA_HEX>
```

`<DATA_HEX>` is the hex-encoded raw bytes of your original data (no `0x`
prefix required by the Soroban CLI).

**Example — hash the file and verify in one step:**

```bash
DATA='{"score":100,"level":"gold"}'
DATA_HEX=$(printf '%s' "$DATA" | xxd -p | tr -d '\n')

soroban contract invoke \
  --id C... \
  -- \
  verify_data \
  --user G... \
  --period 202401 \
  --data "$DATA_HEX"
# → true
```

---

## Matching example

Mint with a specific JSON payload, then verify the same bytes:

```rust
// --- mint ---
let data_json = Bytes::from_slice(&env, b"{\"score\":100,\"level\":\"gold\"}");
let data_hash_raw = env.crypto().sha256(&data_json);
let data_hash = BytesN::from_array(&env, &data_hash_raw.to_array());

client.mint_wrap(
    &user, &period, &archetype,
    &data_hash, &CURRENT_PAYLOAD_VERSION, &signature,
);

// --- verify with the original bytes → true ---
assert!(client.verify_data(&user, &period, &data_json));
```

---

## Non-matching examples

### Tampered field value

```rust
// Minted with {"score":100}
let original = Bytes::from_slice(&env, b"{\"score\":100}");
let data_hash = BytesN::from_array(&env, &env.crypto().sha256(&original).to_array());
client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

// Verifying with {"score":999} returns false — value differs
let tampered = Bytes::from_slice(&env, b"{\"score\":999}");
assert!(!client.verify_data(&user, &period, &tampered));
```

### Whitespace difference

```rust
// Minted with compact JSON (no spaces)
let compact = Bytes::from_slice(&env, b"{\"score\":100}");
let data_hash = BytesN::from_array(&env, &env.crypto().sha256(&compact).to_array());
client.mint_wrap(&user, &period, &archetype, &data_hash, &CURRENT_PAYLOAD_VERSION, &signature);

// Verifying with pretty-printed JSON returns false — byte sequences differ
let pretty = Bytes::from_slice(&env, b"{ \"score\": 100 }");
assert!(!client.verify_data(&user, &period, &pretty));
```

### No wrap exists

```rust
// No mint has been called for this user-period combination
let data = Bytes::from_slice(&env, b"anything");
assert!(!client.verify_data(&user, &202401, &data));
// returns false, does not panic
```

---

## Off-chain workflow summary

```
Off-chain (at mint time)
─────────────────────────────────────────────────
1. Produce canonical JSON bytes  →  raw_bytes
2. sha256(raw_bytes)             →  data_hash
3. Call mint_wrap(..., data_hash, ...)
4. Store raw_bytes alongside the period

Off-chain (at verification time)
─────────────────────────────────────────────────
1. Retrieve raw_bytes for the period
2. Call verify_data(user, period, raw_bytes)
3. true  → on-chain hash matches; data is authentic
   false → data has been altered, or no wrap exists
```

---

## Error reference

`verify_data` itself never panics or returns an error code — it always returns
a `bool`. The only way to get a contract error is if the enclosing transaction
fails for an unrelated reason (e.g. contract not initialised before any query
that requires it). See [ERRORS.md](../ERRORS.md) for the full error catalogue.
