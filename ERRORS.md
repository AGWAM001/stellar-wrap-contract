# Error Reference (ContractError)

Soroban reports contract panics as `Error(Contract, #N)`. The canonical
definitions are in `src/errors.rs`.

| Code | Variant | Meaning |
|---:|---|---|
| 1 | `AlreadyInitialized` | `initialize` was called more than once. |
| 2 | `NotInitialized` | An initialized admin or signing key was required but missing. |
| 3 | `Unauthorized` | Reserved authorization error. Address authorization failures may instead surface as a Soroban host auth error. |
| 4 | `WrapAlreadyExists` | The destination already has a wrap for this period, or the same mint was replayed. |
| 5 | `InvalidSignature` | Reserved signature error. Failed Ed25519 verification may surface as a Soroban crypto host error. |
| 6 | `InvalidPeriod` | The period is not a valid `YYYYMM` value from 202401 through 210012. |
| 7 | `MigrationAlreadyApplied` | The requested migration version is not greater than the stored version. |
| 8 | `WrapNotFound` | `transfer_wrap` could not find the source `(owner, period)` record. |
| 9 | `InvalidTransfer` | The sender and recipient are the same address. |
| 10 | `TransferFeeNotConfigured` | The admin has not called `set_transfer_fee`. |
| 11 | `InvalidFee` | A negative transfer fee was supplied. Zero is valid and explicitly enables fee-free transfers. |
| 12 | `TransferInProgress` | The temporary transfer reentrancy guard is already set. |
| 13 | `StorageInvariantViolation` | Ownership indexes and wrap records disagree, or invalid legacy periods were supplied for backfill. |

## Transfer failures

`transfer_wrap(from, to, period)` performs its checks and token payment in one
Soroban invocation:

- `from` must authorize the call.
- The fee configuration must exist.
- The source record must exist and the destination must not already own that period.
- The configured token contract must successfully transfer the fee from `from`
  to the configured recipient.

Any panic, including insufficient token balance or token authorization failure,
rolls back the fee and all wrap state changes.

Deployments upgraded from a version without `WrapPeriods` must call
`backfill_wrap_periods` for existing owners. Code 13 prevents transfers from
silently corrupting counts or latest-period lookups when that index is absent or
incorrect.

## Mint signature payload

`mint_wrap` verifies an Ed25519 signature over this canonical XDR payload:

```text
contract_address || user || period || archetype || data_hash
```

Including the contract address, user, period, archetype, and data hash prevents
cross-contract and cross-user signature replay.
