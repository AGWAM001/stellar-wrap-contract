# Security Recommendations for Stellar Wrap Contract

## Overview
This document outlines critical security enhancements needed before mainnet deployment. The current implementation has a **stub signature verification** that must be replaced with proper cryptographic verification.

---

## 🔴 CRITICAL: Signature Verification Enhancement

### Current State
The `verify_signature()` function in `lib.rs` currently returns `true` unconditionally:

```rust
fn verify_signature(_data_hash: &BytesN<32>) -> bool {
    true  // ⚠️ INSECURE - Always passes
}
```

### Required Implementation

The signature must cryptographically bind:
1. **User Address** (prevents identity theft)
2. **Contract Address** (prevents cross-contract replay)
3. **Period** (prevents time-based replay)
4. **Data Hash** (prevents data tampering)
5. **Nonce or Sequence** (optional, for additional replay protection)

### Recommended Approach

#### Option 1: Ed25519 Signature Verification (Recommended)

```rust
use soroban_sdk::crypto::ed25519;

fn verify_signature(
    e: &Env,
    admin: &Address,
    user: &Address,
    period: &Symbol,
    data_hash: &BytesN<32>,
    signature: &BytesN<64>
) -> bool {
    // Construct the payload that was signed
    let mut payload = Bytes::new(e);
    
    // Include contract address (prevents cross-contract replay)
    payload.append(&e.current_contract_address().to_bytes());
    
    // Include user address (prevents identity theft)
    payload.append(&user.to_bytes());
    
    // Include period (prevents period replay)
    payload.append(&period.to_bytes());
    
    // Include data hash
    payload.append(&data_hash.to_bytes());
    
    // Hash the payload
    let message = e.crypto().sha256(&payload);
    
    // Get admin's public key and verify signature
    // Note: You'll need to store/retrieve the admin's public key
    let admin_pubkey = get_admin_pubkey(e);
    
    e.crypto().ed25519_verify(
        &admin_pubkey,
        &message,
        signature
    );
    
    true
}
```

#### Option 2: Use Soroban's Built-in Auth (Simpler)

Instead of manual signature verification, leverage Soroban's authorization framework:

```rust
pub fn mint_wrap(
    e: Env,
    to: Address,
    data_hash: BytesN<32>,
    archetype: Symbol,
    period: Symbol,
) {
    let admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

    // This already provides cryptographic verification
    admin.require_auth();
    
    // Rest of implementation...
}
```

**Current code already uses `admin.require_auth()` which is secure!** However, for additional security layers (like binding to specific parameters), you may want to add custom signature verification.

---

## ✅ Security Features Already Implemented

### 1. Replay Attack Protection ✓
**Status:** IMPLEMENTED

The contract prevents replay attacks within the same contract instance through the duplicate check:

```rust
let wrap_key = DataKey::Wrap(to.clone(), period.clone());
if e.storage().instance().has(&wrap_key) {
    panic_with_error!(e, ContractError::WrapAlreadyExists);
}
```

**What it prevents:**
- Same user + same period cannot be minted twice
- Attacker cannot replay a valid transaction

**Test coverage:** 
- `test_replay_attack_same_period_fails` ✓
- `test_duplicate_period_fails` ✓

### 2. Authorization Protection ✓
**Status:** IMPLEMENTED

Only the admin can authorize minting:

```rust
admin.require_auth();
```

**What it prevents:**
- Unauthorized users cannot mint wraps
- Only admin-signed transactions succeed

**Test coverage:**
- `test_mint_wrap_unauthorized` ✓
- `test_non_admin_cannot_mint` ✓

### 3. Initialization Protection ✓
**Status:** IMPLEMENTED

Contract can only be initialized once:

```rust
if e.storage().instance().has(&key) {
    panic_with_error!(e, ContractError::AlreadyInitialized);
}
```

**Test coverage:**
- `test_initialize_twice_fails` ✓

---

## ⚠️ Security Considerations for Deployment

### 1. Cross-Contract Replay Protection
**Status:** NEEDS ENHANCEMENT

**Current behavior:**
- Each contract instance has independent storage
- A signature valid for Contract V1 could theoretically work on Contract V2

**Recommendation:**
If you plan to deploy multiple versions, include `env.current_contract_address()` in the signature payload.

```rust
// In signature verification
let contract_id = e.current_contract_address();
payload.append(&contract_id.to_bytes());
```

**Test coverage:**
- `test_cross_contract_replay_protection` ✓ (documents current behavior)

### 2. Timestamp Integrity
**Status:** SECURE ✓

The contract correctly uses `env.ledger().timestamp()` rather than accepting user-provided timestamps.

**Test coverage:**
- `test_timestamp_is_from_ledger_not_user` ✓

### 3. Identity Binding
**Status:** SECURE (via admin auth) ✓

The current `admin.require_auth()` ensures only authorized transactions succeed. The `to` parameter is part of the storage key, preventing one user from claiming another's wrap.

**Test coverage:**
- `test_signature_cannot_be_stolen_by_another_user` ✓

---

## 📊 Gas/Resource Analysis Results

Run the tests to get exact numbers:

```bash
cargo test test_gas_analysis -- --nocapture
```

Expected output:
```
=== GAS ANALYSIS REPORT ===
Operation: mint_wrap
CPU Instructions: ~[TO BE MEASURED]
Memory Bytes: ~[TO BE MEASURED]
===========================
```

### Optimization Recommendations:
1. **Storage**: Instance storage is used correctly (ephemeral, cheaper than persistent)
2. **Event emission**: Minimal data in events (only period as u64)
3. **Signature verification**: If implementing custom crypto, measure impact

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

- [x] Replay attack protection implemented
- [x] Authorization verification implemented
- [x] Duplicate period prevention implemented
- [x] Timestamp integrity verified
- [x] Comprehensive test suite created
- [ ] **CRITICAL**: Decide on signature verification approach (current admin.require_auth() may be sufficient)
- [ ] **CRITICAL**: If deploying multiple versions, add contract address binding
- [ ] Run gas analysis and document costs
- [ ] Security audit by third party
- [ ] Fuzz testing with property-based tests
- [ ] Load testing for high-volume scenarios

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

### 1. Invariant Testing
Consider adding property-based tests:
- No user should ever have duplicate periods
- Total wraps minted should equal sum of all user wraps
- Timestamps should be monotonic within a session

### 2. Fuzz Testing
Use `cargo-fuzz` to test with random inputs:
```bash
cargo install cargo-fuzz
cargo fuzz init
cargo fuzz run fuzz_target_1
```

### 3. Access Control Review
- Ensure `initialize()` is called during deployment
- Verify admin key is secured in production
- Consider multi-sig admin for production

### 4. Upgrade Strategy
- Plan for contract upgrades if needed
- Consider using a proxy pattern
- Document migration procedures

---

## 🔗 References

- [Soroban Security Best Practices](https://soroban.stellar.org/docs/learn/security)
- [Stellar Smart Contract Audit Guidelines](https://stellar.org/developers)
- [Soroban Auth Framework](https://soroban.stellar.org/docs/learn/authorization)

---

## 📧 Questions?

If implementing custom signature verification, consider:
1. Key management strategy
2. Signature format and standards
3. Off-chain signature generation process
4. Recovery mechanisms

**Current Assessment:** The contract uses Soroban's built-in `require_auth()` which is cryptographically secure and prevents most attack vectors. Additional custom signature verification is optional and depends on your specific security requirements.
