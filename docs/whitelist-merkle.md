# Off-chain whitelisting via Merkle proofs

The contract can gate behaviour on a whitelist of addresses without ever storing
that list on-chain. Only a single 32-byte merkle root is written to instance
storage; membership is proven per-call by the caller supplying a merkle proof.

## Why

Storing N addresses costs N persistent entries plus rent, and every whitelist
edit is another transaction per entry. A merkle commitment collapses that to one
32-byte write, regardless of whether the list has 10 or 10 million members.
Rotating the list is a single `set_whitelist_root` call.

## Leaf encoding

A leaf is the SHA-256 of a domain separator concatenated with the XDR encoding
of the address:

```
leaf = SHA256( "stellar-wrap-whitelist-v1" || XDR(ScVal::Address(user)) )
```

The domain separator (`merkle::WHITELIST_DOMAIN_SEPARATOR`) makes a whitelist
leaf structurally unable to collide with a mint payload (`stellar-wrap-v1`, see
[`signing-payload.md`](./signing-payload.md)) or with a batch-claim leaf, even
though all three are SHA-256 digests.

`whitelist_leaf(user)` returns the on-chain leaf for an address so off-chain
tooling can assert byte-for-byte parity before publishing a root.

## Tree layout

Identical to the claim tree built by [`scripts/merkle.ts`](../scripts/merkle.ts):

- Binary tree, built bottom-up from the leaf array.
- Internal node = `SHA256(min(a, b) || max(a, b))` — siblings are sorted
  lexicographically, so a proof does not need left/right position flags.
- An odd node at the end of a layer is promoted unchanged to the next layer.

Because pairs are sorted, `verify_merkle_proof` walks the proof in order and
folds each sibling into the running hash:

```
computed = leaf
for sibling in proof:      // ordered leaf-sibling → root-sibling
    computed = SHA256(min(computed, sibling) || max(computed, sibling))
accept if computed == root
```

## Contract API

| Function | Auth | Purpose |
| --- | --- | --- |
| `set_whitelist_root(root)` | admin | Publish/replace the root. Emits `("whitelist", "root")`. |
| `clear_whitelist_root()` | admin | Delete the root, disabling whitelist checks. Emits `("whitelist", "cleared")`. |
| `get_whitelist_root()` | — | Current root, or `None`. |
| `whitelist_leaf(user)` | — | Leaf hash for an address. |
| `verify_whitelist(user, proof)` | — | `true` if `proof` proves membership. |

Errors:

- `MerkleRootNotSet` (15) — no root published yet.
- `InvalidMerkleProof` (16) — raised by the internal `require_whitelisted`
  gate when a proof does not verify. `verify_whitelist` returns `false`
  instead of panicking so it is usable as a read-only query.

## Using it as a gate

`merkle::require_whitelisted(&e, &user, &proof)` is the panicking form intended
for embedding in other entrypoints (for example, gating minting to whitelisted
users during a private phase). It panics with `InvalidMerkleProof` on failure so
the whole transaction reverts.

## Security notes

- The root is only as trustworthy as the admin that publishes it; rotating the
  root under the timelock (`TimelockAction::SetWhitelistRoot`) makes whitelist
  changes observable in advance — see [`timelock.md`](./timelock.md).
- Sorted-pair hashing removes position flags but means a tree must never contain
  duplicate leaves; the off-chain builder should deduplicate addresses.
- Proofs are public data. Whitelist membership is not confidential — the root
  hides the *list*, not the fact that a given address is on it.
- Proof length grows as `log2(N)`; verification cost is one SHA-256 per proof
  element, so callers pay for their own membership check.
