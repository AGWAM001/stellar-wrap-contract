# Admin Rotation Procedure

This document describes the safe procedure for rotating admin control of the
Stellar Wrap Contract. Operators should follow it any time the admin address or
the Ed25519 signing pubkey needs to change.

---

## Two independent rotation types

The contract stores two separate privileged values. Rotating one does **not**
affect the other.

| Value | Storage key | Controls |
|---|---|---|
| Admin address | `DataKey::Admin` | Authorization for all admin-only functions (`update_admin`, `pause`, `upgrade`, `migrate`, …) |
| Signing pubkey | `DataKey::AdminPubKey` | Ed25519 public key used to verify mint payload signatures |

---

## Admin address rotation

### Preparation

1. Confirm the **current** admin on-chain before starting:
   ```bash
   stellar contract invoke \
     --id <CONTRACT_ID> \
     --network <NETWORK> \
     -- get_admin
   ```
2. Have the new admin verify they control their private key by signing a test
   transaction on testnet first.
3. Ensure no pending proposal already exists:
   ```bash
   stellar contract invoke \
     --id <CONTRACT_ID> \
     --network <NETWORK> \
     -- get_pending_admin
   ```
   If a proposal exists, cancel it before proceeding (see
   [Cancel a pending proposal](#cancel-a-pending-proposal)).

### Ruta A — Direct transfer (`update_admin`)

> **Warning:** This path is immediate and irreversible in the same transaction.
> Only use it when the new admin address has already been verified on testnet.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network <NETWORK> \
  --source <CURRENT_ADMIN_SECRET> \
  -- update_admin \
  --new_admin <NEW_ADMIN_ADDRESS>
```

The contract replaces the stored admin address and emits an
`("admin", "updated")` event immediately.

### Ruta B — Two-step transfer (`propose_admin` + `accept_admin`) — recommended

This path keeps the current admin in control until the new admin explicitly
accepts, protecting against address typos and key-loss scenarios.

**Step 1 — Current admin proposes a new admin:**
```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network <NETWORK> \
  --source <CURRENT_ADMIN_SECRET> \
  -- propose_admin \
  --new_admin <NEW_ADMIN_ADDRESS>
```

**Step 2 — Verify the pending admin is correct:**
```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network <NETWORK> \
  -- get_pending_admin
```

Confirm the returned address matches the intended new admin before proceeding.

**Step 3 — New admin accepts:**
```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network <NETWORK> \
  --source <NEW_ADMIN_SECRET> \
  -- accept_admin
```

The contract atomically moves `PendingAdmin` → `Admin` and clears the pending
slot. An `("admin", "updated")` event is **not** emitted by `accept_admin`;
monitor via `get_admin` (see [Verification](#verification)).

### Cancel a pending proposal

If a mistake is detected **after** `propose_admin` but **before** `accept_admin`,
the current admin can cancel:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network <NETWORK> \
  --source <CURRENT_ADMIN_SECRET> \
  -- cancel_proposed_admin
```

Only one proposal may be open at a time. `propose_admin` will panic with
`AdminTransferProposalExists` if you try to open a second one without cancelling
the first.

---

## Verification

Run these queries after every rotation to confirm the expected state:

```bash
# 1. Confirm new admin is active
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network <NETWORK> \
  -- get_admin

# 2. Confirm no pending proposal remains
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network <NETWORK> \
  -- get_pending_admin

# 3. Confirm contract health (initialized, has_admin, has_signing_key)
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network <NETWORK> \
  -- health

# 4. Smoke-test admin access with a no-op privileged call (e.g. unpause if paused, or pause+unpause)
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network <NETWORK> \
  --source <NEW_ADMIN_SECRET> \
  -- is_paused
```

---

## Event monitoring

A successful `update_admin` call emits one event:

- **Topic 0:** `admin` (`Symbol`)
- **Topic 1:** `updated` (`Symbol`)
- **Data:** `(old_admin, new_admin)` — both as `Address`

Indexers can watch for this event to detect admin rotations without polling, but
must always verify the live admin via `get_admin` before enforcing privileged
flows. **Do not infer state from events alone.**

`accept_admin` does not currently emit an event; use `get_admin` to confirm.

---

## Signing pubkey rotation

The Ed25519 signing pubkey (`DataKey::AdminPubKey`) is set during `initialize`
and is **not** exposed via a dedicated rotation function. Rotating it requires a
contract upgrade that includes a migration:

1. Deploy a new WASM version that exposes an `update_admin_pubkey` function (or
   equivalent migration path).
2. Call `upgrade` with the new WASM hash (requires admin auth):
   ```bash
   stellar contract invoke \
     --id <CONTRACT_ID> \
     --network <NETWORK> \
     --source <ADMIN_SECRET> \
     -- upgrade \
     --new_wasm_hash <NEW_WASM_HASH_HEX>
   ```
3. Call `migrate` with the new migration version to apply the key change:
   ```bash
   stellar contract invoke \
     --id <CONTRACT_ID> \
     --network <NETWORK> \
     --source <ADMIN_SECRET> \
     -- migrate \
     --version <NEXT_MIGRATION_VERSION>
   ```
4. Verify with `health` that `has_signing_key` is still `true` and test a mint
   with a payload signed by the new key on testnet before mainnet.

> **Key difference from admin address rotation:** rotating the signing pubkey
> requires a contract upgrade and is a more involved process. Plan for a
> maintenance window and test thoroughly on testnet.

---

## Rollback plan

| Scenario | Recovery action |
|---|---|
| Wrong address proposed, not yet accepted (Ruta B) | Current admin calls `cancel_proposed_admin` |
| Tx not sent yet (Ruta A) | Do not broadcast; re-run with correct address |
| Wrong admin active, new admin **cooperates** | New admin calls `update_admin` to the correct address |
| Wrong admin active, new admin **does not cooperate** | No on-chain recovery — the contract remains under the wrong admin's control |

**The safest default is always Ruta B (propose + accept).** It provides a
cancel window and requires the new admin to prove key control before the old
admin loses access.

For mainnet rotations, always rehearse the full procedure on testnet with the
exact addresses involved before executing on mainnet.
