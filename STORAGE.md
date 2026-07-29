# Storage accounting and algorithmic fee

This contract now keeps a conservative estimated count of persistent storage bytes in instance storage and exposes an on-chain algorithmic fee computed from that estimate.

Key details:

- DataKey::StorageBytes (instance) stores a u64 estimate of persistent bytes.
- DataKey::FeeParams (instance) stores a FeeParams struct with these fields:
  - base_fee: i128
  - per_kib_fee: i128
  - scale_step_kib: u64
  - max_fee: i128

Fee formula (on-chain):

fee = min(max_fee, base_fee + per_kib_fee * ceil(storage_bytes / 1024 / scale_step_kib))

Accounting updates:

- mint_wrap() increments estimate when creating new persistent entries (wrap record, wrap count, latest, user_periods) using conservative constants.
- revoke_wrap() decrements estimate for the removed wrap and for the wrap-count entry when it reaches zero.

Notes:

- Because Soroban does not expose raw ledger storage size to contracts, this approach uses contract-side accounting and conservative estimates. To bootstrap existing deployments, use an admin migration that recomputes storage_bytes by scanning known indexes in bounded chunks (not provided here).
