# Security Recommendations for Stellar Wrap Contract

## Overview
This document describes the security properties of the deployed contract, identifies
remaining pre-mainnet work, and provides guidance for auditors.

For developer-facing error handling (e.g. `Error(Contract, #4)`), see the
[Error Reference (ERRORS.md)](./ERRORS.md).

---

## ✅ Signature Verification — Implemented (Ed25519)

### Current State
`mint_wrap()` and `update_wrap()` perform real Ed25519 cryptographic signature
verification using Soroban's built-in `e.crypto().ed25519_verify()`. The old
unconditional stub (`fn verify_signature() -> bool { true }`) no longer exists.

### How It Works

The backend generates a **canonical payload** and signs it with the admin Ed25519
private key. The contract reconstructs the same payload and verifies the signature
against the stored admin public key before minting.

#### Payload construction (`mint_wrap`)

```
payload = 0x01
        ‖ XDR(contract_address)
        ‖ XDR(user)
        ‖ XDR(period)        // u64 — prevents period replay
        ‖ XDR(archetype)
        ‖ XDR(data_hash)     // SHA-256 of off-chain JSON
```

The first byte is a payload version field. The contract currently accepts version `1` only, and backend signers must use this version for all new signatures.

Each field is XDR-encoded before concatenation, which provides unambiguous
length-delimited framing and prevents field-boundary collisions.

#### On-chain verification (Rust)

```rust
let mut payload = Bytes::new(&e);
payload.append(&e.current_contract_address().to_xdr(&e));   // cross-contract replay protection
payload.append(&user.clone().to_xdr(&e));                   // identity binding
payload.append(&period.to_xdr(&e));                         // period binding
payload.append(&archetype.clone().to_xdr(&e));
payload.append(&data_hash.clone().to_xdr(&e));

// Panics with ContractError::InvalidSignature (code 6) on failure
e.crypto().ed25519_verify(&admin_pubkey, &payload, &signature);
```

`ed25519_verify` panics (traps) when verification fails — the transaction is
rolled back and no state is written.

#### Off-chain signing (TypeScript example)

```typescript
import { xdr, Address } from "@stellar/stellar-sdk";
import * as nacl from "tweetnacl";

function buildMintPayload(
  contractId: string,
  userAddress: string,
  period: bigint,
  archetype: string,
  dataHash: Uint8Array
): Uint8Array {
  const parts: Uint8Array[] = [
    xdr.ScAddress.scAddressTypeContract(Buffer.from(contractId, "hex")).toXDR(),
    Address.fromString(userAddress).toXDR(),
    xdr.Uint64.fromBigInt(period).toXDR(),
    xdr.ScSymbol.encode(archetype),
    Buffer.from(dataHash),
  ];
  return Buffer.concat(parts);
}

const payload  = buildMintPayload(contractId, user, period, archetype, dataHash);
const signature = nacl.sign.detached(payload, adminSecretKey);
// Pass signature (64 bytes) as the `signature` argument to mint_wrap()
```

### Security Properties Provided

| Property | How it is enforced |
|---|---|
| **Identity binding** | `user` is in the payload; a signature for Alice cannot be used by Bob |
| **Cross-contract replay** | `contract_address` is the first payload field |
| **Period replay** | `period` is in the payload; same user cannot reuse a signature for a different period |
| **Data integrity** | `data_hash` (SHA-256 of JSON) is in the payload and also stored on-chain |
| **Duplicate prevention** | `WrapAlreadyExists` check after signature verification |

---

## ✅ Security Features Already Implemented

### 1. Replay Attack Protection
**Status:** IMPLEMENTED

```rust
let wrap_key = DataKey::Wrap(user.clone(), period);
if e.storage().persistent().has(&wrap_key) {
    panic_with_error!(e, ContractError::WrapAlreadyExists);
}
```

The same `(user, period)` pair can never be minted twice regardless of whether the
caller has a valid signature.

**Test coverage:**
- `test_replay_attack_same_period_fails` ✓
- `test_duplicate_period_fails` ✓

### 2. Authorization Protection
**Status:** IMPLEMENTED

```rust
user.require_auth();       // user must sign the Stellar transaction
// … then …
e.crypto().ed25519_verify(&admin_pubkey, &payload, &signature);  // admin must pre-sign payload
```

Both conditions must hold. An attacker who controls the Stellar keypair of a user
still cannot forge the Ed25519 admin signature, and vice-versa.

**Test coverage:**
- `test_mint_wrap_unauthorized` ✓
- `test_non_admin_cannot_mint` ✓

### 3. Initialization Protection
**Status:** IMPLEMENTED

```rust
if e.storage().instance().has(&DataKey::Admin) {
    panic_with_error!(e, ContractError::AlreadyInitialized);
}
```

**Test coverage:**
- `test_initialize_twice_fails` ✓

### 4. Reentrancy Guard
**Status:** IMPLEMENTED

A temporary-storage guard (`DataKey::MintGuard`) is set at the start of `mint_wrap`
and removed on success. If execution panics mid-flight, Soroban's ledger rollback
prevents the guard from persisting incorrectly.

### 5. Zero-Hash Rejection
**Status:** IMPLEMENTED

```rust
if data_hash == BytesN::from_array(&e, &ZERO_HASH_BYTES) {
    panic_with_error!(e, ContractError::InvalidDataHash);
}
```

All-zero `data_hash` values are rejected to guard against missing or
uninitialized data.

### 6. Timestamp Integrity
**Status:** SECURE

The contract uses `e.ledger().timestamp()` rather than accepting a
user-supplied timestamp. Ledger timestamps are set by consensus and cannot be
forged by a transaction submitter.

**Test coverage:**
- `test_timestamp_is_from_ledger_not_user` ✓

### 7. Archetype Allowlist
**Status:** IMPLEMENTED

Archetypes are validated against an admin-managed allowlist stored in instance
storage. Unknown archetypes are rejected with `InvalidArchetype`.

```rust
Self::validate_archetype(&e, &archetype);
```

### 8. Cross-Contract Replay Protection
**Status:** IMPLEMENTED

`e.current_contract_address()` is the first field in every signed payload. A
signature issued for Contract V1 will fail verification on Contract V2 (different
address) even if all other fields are identical.

**Test coverage:**
- `test_cross_contract_replay_protection` ✓

---

## ⚠️ Remaining Pre-Mainnet Items

### 1. Signature Expiry / `expiry_ledger`
**Status:** PARTIALLY IMPLEMENTED

`mint_wrap` accepts an `expiry_ledger: u32` parameter and `sign_payload` includes
it in the payload in tests. The on-chain check that rejects signatures past their
expiry ledger should be confirmed present and tested end-to-end before mainnet.

**Recommendation:** Verify that `mint_wrap` explicitly compares
`e.ledger().sequence() > expiry_ledger` and panics with an appropriate error.
Add a test that a signature with a past `expiry_ledger` is rejected.

### 2. Admin Key Management
**Status:** OPERATIONAL CONCERN

The `admin_pubkey` is stored in instance storage and verified on every mint. The
corresponding private key must be held securely (HSM or similar). Key rotation
requires `update_admin()` followed by redeployment of a new signing service.

**Recommendation:** Document the key rotation procedure and rehearse it before
mainnet.

### 3. Third-Party Security Audit
**Status:** PENDING

**Recommendation:** Engage an independent Soroban auditor before handling real
user data at scale.

### 4. Fuzz Testing
**Status:** PENDING

Consider adding property-based or fuzz tests:
- No user should have duplicate periods.
- Sum of all `WrapCount` values should equal `TotalMints`.
- Any random 64-byte blob passed as `signature` must be rejected.

```bash
cargo install cargo-fuzz
cargo fuzz init
cargo fuzz run fuzz_target_1
```

### 5. Upgrade Key Control
**Status:** OPERATIONAL CONCERN

`upgrade()` requires admin authorization. If the admin keypair is compromised, an
attacker could replace the WASM. Consider a time-lock or multi-sig admin for
production.

---

## 🔄 Payload Versioning & Backend Migration Process (#274)

### Why Payload Versioning Exists

The canonical payload format may evolve (e.g., adding new signed fields like `metadata`,
`archetype_allowlist_revision`, or `expiry_ledger`). Without a version prefix,
a signature produced against an older payload schema could be successfully
re-submitted to a newer contract if the remaining fields happen to match — a
cross-version replay attack.

This contract defends against this by:

1. Prepending `payload_version: u32` as the **first** element of every signed
   payload.
2. Rejecting any `mint_wrap` call where the provided `payload_version` does not
   equal `CURRENT_PAYLOAD_VERSION` (currently `1`) — it panics with
   `Error(Contract, #5)` / ContractError::InvalidSignature`).

**On-chain payload (v1):
```
payload = XDR(payload_version: u32)
        ‖ XDR(contract_address)
        ‖ XDR(user)
        ‖ XDR(period)
        ‖ XDR(archetype)
        ‖ XDR(data_hash)
```

### On-chain Version Evolution Rules
- **Rust location:** `src/mint.rs:8` (`CURRENT_PAYLOAD_VERSION = 1`)
- **Validation:** `src/mint.rs:19-23` (`validate_payload_version`)
- **Prepend order:** `src/mint.rs:42` (version is the first field in the payload)

### Backend Migration Checklist

#### Step 1 — Contract upgrade (Soroban CLI)

Bump the version number in `src/mint.rs:8` (`CURRENT_PAYLOAD_VERSION = 1 -> 2`),
then redeploy the WASM via `soroban contract upgrade`, and finally update the payload schema
(see `Makefile` targets. Deploy via `soroban contract upgrade`).

#### Step 2 — Backend signing code (pdate
When the new contract lands on-chain, update the backend signer to prepend the new
version:

```python
# BEFORE (no version prefix)
payload = (
    contract.to_xdr()
    + user.to_xdr()
    + period.to_xdr()
    + archetype.to_xdr()
    + data_hash.to_xdr()
)
signature = admin_ed25519_sign(payload)

# AFTER — prepend payload_version as a LE u32 XDR-encoded
payload_version = 2  # MUST match CURRENT_PAYLOAD_VERSION in src/mint.rs
payload = (
    xdr_u32(payload_version)
    + contract.to_xdr()
    + user.to_xdr()
    + period.to_xdr()
    + archetype.to_xdr()
    + data_hash.to_xdr()
)
signature = admin_ed25519_sign(payload)
```

```
* Order matters: payload_version MUST be the first bytes in the serialized buffer,
because the contract appends it in `src/mint.rs:42`.

#### Step 3 — Cutover & dual-signing)

To avoid a race windows during the WASM upgrade:
1. Roll out backend code accepts old-style signatures to accept both versions while the network still only
   old contract:
     - *Option A — Deploy in two phases:
     first deploy a "accepts both v1 and v2 signatures (by temporarily
        wideningvalidate_payload_version to accept an allow list), then
        swap back once backend clients.
     - *Option B (simpler, recommended — Perform the contract upgrade in a
        single ledger window; do not send mints during the swap.  Ledger
        sequenced to
#### Step 4 ) Verify against replay safety
After upgrade:
```bash
cargo test test_cross_version_replay  # run the version-gating tests added
cargo test test::test_same_version_sig_succeeds
```

Tests added covering the attack vectors:

| Test | Attack | Result |
|------|--------|--------|
| `test_cross_version_replay_v0_sig_submitted_as_v1_fails` | Sign with v0 = 0; submit with v=CURRENT=1 | PANIC #5 |
| `test_cross_version_replay_v2_sig_submitted_as_v1_fails` | Sign with v2; submit with v=CURRENT=1 | PANIC #5 |
| `test_same_version_sig_succeeds` | Sign and submit same v=CURRENT | SUCCESS |
| `test_wrong_payload_version_alone_fails_even_with_matching_sig` | Sign v=CURRENT but submit v=99 | PANIC #5 |

---

## 📊 Gas / Resource Analysis

```bash
# Run gas-annotated tests
cargo test test_gas_analysis -- --nocapture
```

### Known cost breakdown

| Operation | Notable costs |
|---|---|
| `mint_wrap` | 2 persistent writes (`WrapRecord` + `WrapCount`), 1 temp write/remove (guard), 1 event |
| `has_wrap` | 1 persistent `has()` call — no deserialization, cheaper than `get_wrap` |
| `get_wrap` | 1 persistent read + XDR deserialization of `WrapRecord` |
| `balance_of` | 1 persistent read (`WrapCount`) |

---

## 🧪 Test Suite Summary

### Security Tests (`src/security_test.rs`)

| Test | Purpose | Expected Behavior |
|------|---------|-------------------|
| `test_replay_attack_same_period_fails` | Replay protection | PANIC #4 |
| `test_replay_attack_different_hash_same_period_fails` | Duplicate period prevention | PANIC #4 |
| `test_multiple_periods_for_same_user_success` | Valid multi-period usage | SUCCESS |
| `test_signature_cannot_be_stolen_by_another_user` | Identity theft prevention | SUCCESS (isolation) |
| `test_cross_contract_replay_protection` | Cross-contract isolation | SUCCESS (independent storage) |
| `test_gas_analysis_mint_operation` | Resource consumption | Prints metrics |
| `test_gas_analysis_multiple_mints` | Scaling analysis | Prints metrics |
| `test_timestamp_is_from_ledger_not_user` | Timestamp integrity | SUCCESS |
| `test_edge_case_long_symbols` | Symbol length limits | SUCCESS |
| `test_non_admin_cannot_mint` | Authorization check | PANIC |

### Running Tests

```bash
# Run all tests
cargo test

# Run only security tests
cargo test security_test

# Run with output for gas analysis
cargo test test_gas_analysis -- --nocapture

# Run with detailed output
cargo test -- --nocapture --test-threads=1
```

---

## 🚀 Pre-Mainnet Checklist

- [x] Ed25519 signature verification implemented (`e.crypto().ed25519_verify`)
- [x] Replay attack protection implemented (`WrapAlreadyExists` check)
- [x] Admin authorization implemented (`require_auth`)
- [x] Duplicate period prevention implemented
- [x] Timestamp integrity — uses ledger timestamp, not user-supplied
- [x] Reentrancy guard (temporary storage `MintGuard`)
- [x] Zero data_hash rejection (`InvalidDataHash`)
- [x] Cross-contract replay protection (contract address in payload)
- [x] Archetype allowlist validation
- [x] Comprehensive unit + security test suite
- [ ] Confirm `expiry_ledger` check is enforced on-chain and tested
- [ ] Rehearse admin key rotation procedure
- [ ] Run gas analysis and document costs for each entry point
- [ ] Third-party security audit
- [ ] Fuzz testing with property-based tests
- [ ] Load/stress testing for high-volume scenarios

---

## 🔒 Issue #31: One-Signature-Per-Period Invariant

### Overview

The duplicate check in `mint_wrap` uses `DataKey::Wrap(user, period)` as the deduplication key.
This means:

- Once a user successfully mints for a given period, **no further mint for that period is ever
  accepted**, regardless of the archetype or data hash carried in the new call.
- A signature for `(user, period, archetype_A)` and a second signature for
  `(user, period, archetype_B)` cannot both be consumed — the first one to land wins, and the
  second becomes permanently orphaned.

### The Invariant

> **The admin backend MUST issue exactly one signature per `(user, period)` pair.**

This is an off-chain operational invariant, not enforced by the contract. The contract enforces
that at most one record can be stored per `(user, period)`, but it cannot prevent the backend from
accidentally generating multiple valid signatures before the first is consumed.

### Why Orphaned Signatures Are a Liability

If two valid signatures for the same `(user, period)` exist but only one can ever be used:

1. **Upgrade risk:** If contract logic changes (e.g., the deduplication key is widened or the
   archetype is added to the storage key), the previously orphaned signature becomes usable and
   could write an unexpected record.
2. **Information leakage:** A second signature reveals that the admin considered a different
   archetype for the user, which may be undesirable.
3. **Key rotation edge cases:** If the admin key is rotated and old signatures are not explicitly
   revoked, orphaned signatures from the old key remain valid until the key is removed.

### Mitigation Options

#### Adopted: Document and Enforce Off-Chain

The simplest and currently adopted approach is to **document this invariant here** and ensure the
backend service guarantees it:

- Compute and sign exactly one `(user, period, archetype, data_hash)` tuple per period.
- Never re-sign with a different archetype if a signature has already been issued for that period.
- Maintain an idempotency log: if a period is requested again, return the existing signature, never
  generate a new one.

#### Optional: Nonce-Based Replay Protection

For stronger guarantees a nonce can be added to the signed payload and stored on-chain:

```rust
// In DataKey:
NonceUsed(Address, u64),   // user + period → bool

// In mint_wrap, after signature verification:
let nonce_key = DataKey::NonceUsed(user.clone(), period);
if e.storage().persistent().has(&nonce_key) {
    panic_with_error!(e, ContractError::WrapAlreadyExists);
}
e.storage().persistent().set(&nonce_key, &true);
```

This is redundant with the existing `Wrap(user, period)` key check for the current storage schema,
but becomes valuable if the schema ever changes to allow multiple records per period.

#### Optional: Include Archetype in the Storage Key

If the design goal ever changes to allow one wrap per `(user, period, archetype)` triple, update
`DataKey::Wrap` to `DataKey::Wrap(Address, u64, Symbol)`. This would make every archetype-specific
signature independently consumable, but would also allow a user to mint multiple records for the
same period, which contradicts the SBT one-record-per-period model.

### Current Status

- ✅ At most one record per `(user, period)` — enforced on-chain.
- ✅ Cross-contract replay protection — contract address is included in the signed payload.
- ✅ Cross-period replay protection — period is included in the signed payload.
- ⚙️  One-signature-per-period — enforced **off-chain** by the backend (documented invariant).

---

## 🕒 TTL Lifecycle & Data Freshness

### Current TTL Strategy

All persistent storage entries are created with a TTL of **~1 year** (17280 × 365 ledgers):

| Key | TTL Set At | Auto-Renewed On Mint? |
|-----|-----------|----------------------|
| `Wrap(user, period)` | `mint_wrap` | ❌ No — fixed at creation |
| `WrapCount(user)` | `mint_wrap` | ✅ Yes — extended on every mint |
| `LatestPeriod(user)` | `mint_wrap` | ✅ Yes — extended on every mint |
| Contract instance | `extend_ttl` / `renew_all_ttls` | ✅ Yes — extended on every mint (via metadata keys) |

### Design Decision: Auto-Renew Metadata Only

**Chosen approach:** Auto-renew `WrapCount` and `LatestPeriod` metadata on every `mint_wrap`, but **not** individual historical wrap records.

**Rationale:**
- Metadata keys are small, cheap to extend, and essential for core queries (`balance_of`, `get_latest_wrap`)
- Historical wraps are numerous — iterating them on every mint would be expensive (see gas analysis)
- Full wrap enumeration requires period tracking, tracked as [Issue #90](https://github.com/zintarh/stellar-wrap-contract/issues/90)

**Tradeoffs:**
- ✅ Active users' metadata stays alive automatically
- ✅ New wraps are always fully covered
- ✅ Gas cost per mint is bounded and predictable
- ❌ Historical wraps of long-active users could expire after ~1 year
- ❌ Requires off-chain bots or admin to call `extend_ttl` for old periods of active users
- ❌ Without [#90](https://github.com/zintarh/stellar-wrap-contract/issues/90), there is no way to enumerate a user's periods on-chain

### Mitigation Recommendations

1. **Off-chain renewal bot:** Run a cron job that calls `extend_ttl(user, period)` for all periods of users who have minted in the last 6 months
2. **Admin bulk renewal:** Call `renew_all_ttls(user)` periodically for active users to renew their metadata keys
3. **Future enhancement:** Implement period enumeration ([#90](https://github.com/zintarh/stellar-wrap-contract/issues/90)) to enable full auto-renewal on mint

### Gas Analysis: Auto-Renewal Cost

| Operation | Cost (CPU instructions) |
|-----------|------------------------|
| Single `extend_ttl` for 1 wrap | ~[TBD — run `test_gas_analysis`] |
| Single `extend_ttl` for 5 wraps | ~[TBD — run `test_gas_analysis`] |
| Auto-renew metadata in `mint_wrap` | Already included in mint cost (3 extend_ttl calls) |
| 10 historical wraps + metadata | ~10× single wrap cost |

> **Current implementation already extends 3 keys** on every mint (new wrap, WrapCount, LatestPeriod). Extending N additional historical wraps would add N× the cost of a single `extend_ttl`. For a user with 12 monthly wraps, auto-renewing all 12 would cost ~4× the current mint cost.

### Test Coverage for TTL

| Test | Purpose |
|------|---------|
| `test_metadata_ttl_extended_on_new_mint` | Verifies `WrapCount` and `LatestPeriod` survive after multiple mints |
| `test_old_wrap_preserved_on_new_mint` | Verifies old wraps are not lost when new wraps are minted |
| `test_renew_all_ttls_extends_metadata` | Verifies admin bulk-renewal works |
| `test_renew_all_ttls_requires_admin_auth` | Verifies admin authorization is required |
| `test_renew_all_ttls_before_init_fails` | Verifies failure before initialization |

---
## 📚 Additional Security Best Practices

### Invariant Testing

Consider property-based tests verifying:
- No user ever has duplicate periods.
- Total wraps minted equals sum of all user `WrapCount` values.
- Timestamps are monotonic within a session.

### Access Control Review

- Confirm `initialize()` is called exactly once during deployment.
- Verify admin key is stored securely (HSM or equivalent) in production.
- Consider multi-sig admin (`require_auth_for_args`) for production upgrade control.

---

## 🔗 References

- [Soroban Security Best Practices](https://soroban.stellar.org/docs/learn/security)
- [Stellar Smart Contract Audit Guidelines](https://stellar.org/developers)
- [Soroban Auth Framework](https://soroban.stellar.org/docs/learn/authorization)
- [Soroban `ed25519_verify` docs](https://docs.rs/soroban-sdk/latest/soroban_sdk/crypto/struct.Crypto.html)
