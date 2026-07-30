# Stellar Wrap Contract

Soroban contract for storing Stellar Wrap records by wallet, reporting successful
mints, and supporting owner-authorized transfers with an admin-configured token fee.

## Contract layout

The contract is split into focused modules:

- `src/lib.rs`: contract type and module wiring
- `src/admin.rs`: initialization and admin updates
- `src/mint.rs`: period validation, signature verification, wrap minting, event emission
- `src/transfer.rs`: owner-authorized transfers, fee collection, and ownership indexes
- `src/queries.rs`: read-only queries and metadata
- `src/errors.rs`: contract error codes
- `src/storage_types.rs`: storage keys and persisted record types
- `src/test_utils.rs`: shared test-only helpers (e.g. payload signing)

## Data model

### `WrapRecord`

Each wrap record stores:

- `timestamp: u64`
- `data_hash: BytesN<32>`
- `archetype: Symbol`
- `period: u64`

`period` is encoded as `YYYYMM` and validated on mint:

- year must be between `2024` and `2100`
- month must be between `01` and `12`

## Transfer model

Wrap records are registry entries rather than fungible tokens. The contract omits
`transfer_from`, `approve`, and `allowance`; an owner can move only a specific
`(owner, period)` record by calling `transfer_wrap` and authorizing the call.

Transfers use an admin-controlled `TransferFeeConfig`:

- `token` is the Soroban token contract used for payment
- `recipient` receives the fee
- `amount` is charged to the current owner; zero explicitly enables fee-free transfers

The token payment and ownership update are one atomic Soroban invocation. If the
fee payment fails, the wrap remains with its current owner. The record's original
timestamp, data hash, archetype, and period are preserved.
The owner's authorization covers both the `transfer_wrap` invocation and its
nested token transfer; the contract never holds a token allowance.

### `ContractHealth`

Returned by `health()`, reports:

- `initialized: bool` — whether `initialize()` has been called
- `has_admin: bool` — whether an admin address is currently configured
- `has_signing_key: bool` — whether an admin signing key is currently configured

## Storage keys

- `DataKey::Admin`
- `DataKey::AdminPubKey`
- `DataKey::Wrap(Address, u64)`
- `DataKey::WrapCount(Address)`
- `DataKey::LatestPeriod(Address)`
- `DataKey::WrapPeriods(Address)`
- `DataKey::TransferFee`
- `DataKey::TransferGuard` (temporary storage)
- `DataKey::MigrationVersion`

## Public interface

### Write methods

- `initialize(e: Env, admin: Address, admin_pubkey: BytesN<32>)`
- `update_admin(e: Env, new_admin: Address)`
- `set_transfer_fee(e: Env, token: Address, recipient: Address, amount: i128)`
- `mint_wrap(e: Env, user: Address, period: u64, archetype: Symbol, data_hash: BytesN<32>, signature: BytesN<64>)`
- `transfer_wrap(e: Env, from: Address, to: Address, period: u64)`
- `backfill_wrap_periods(e: Env, user: Address, periods: Vec<u64>)`
- `migrate(e: Env, version: u32)`

### Read methods

- `get_wrap(e: Env, user: Address, period: u64) -> Option<WrapRecord>`
- `balance_of(e: Env, user: Address) -> i128`
- `verify_data(e: Env, user: Address, period: u64, data: Bytes) -> bool`
- `get_latest_wrap(e: Env, user: Address) -> Option<WrapRecord>`
- `get_admin(e: Env) -> Option<Address>`
- `get_transfer_fee(e: Env) -> Option<TransferFeeConfig>`
- `health(e: Env) -> ContractHealth`
- `name(e: Env) -> String`
- `symbol(e: Env) -> String`
- `decimals(e: Env) -> u32`
- `migration_version(e: Env) -> u32`

## Event schemas

### Mint event

Successful wrap mints emit one event:

- **Topic 0**: `mint` (`Symbol`)
- **Topic 1**: `user` (`Address`) - The wallet address that received the wrap
- **Topic 2**: `period` (`u64`) - The period in `YYYYMM` format (e.g., `202401`)
- **Data**: `archetype` (`Symbol`) - The wrap archetype identifier

**Example values:**
- Topic 0: `mint`
- Topic 1: `GD5...` (32-byte Stellar address)
- Topic 2: `202401`
- Data: `arch` (or any short symbol)

**Properties relevant to indexers:**
- The event is emitted only after signature verification and storage writes succeed
- Duplicate `(user, period)` mints are rejected, so one event equals one successful new wrap
- `period` is always a validated `YYYYMM` value (year: 2024-2100, month: 01-12)

### Admin update event

The `update_admin` function does **not** emit an event. To track admin changes, indexers should:
- Query the `get_admin(e)` function periodically
- Store the current admin address and detect changes across queries

### Transfer event

Successful transfers emit:

- **Topic 0**: `transfer`
- **Topic 1**: `from`
- **Topic 2**: `to`
- **Topic 3**: `period`
- **Data**: `(fee_token, fee_recipient, fee_amount)`

The event is emitted only after fee collection and all ownership indexes have
been updated.

### Transfer-fee configuration event

`set_transfer_fee` emits topics `(fee_set, token, recipient)` with `amount` as
event data.

## Important note for indexers

**⚠️ Do not infer state from events alone.** Use contract queries to verify wrap existence:
- `get_wrap(e, user, period)` to retrieve full wrap record
- `balance_of(e, user)` to get total wrap count for a user

## Leaderboard decision

Issue `#68` is implemented as an off-chain leaderboard strategy.

Reasoning:

- Soroban storage does not support efficient range scans for ranking
- maintaining an on-chain sorted top-N list would add write amplification and higher gas costs to every mint
- indexers already need mint events for analytics, so leaderboard aggregation fits the existing data flow

Recommended aggregation rule:

1. index every `mint` event
2. group by topic 1 (`user`)
3. count events per user
4. sort descending by count to produce the leaderboard

## Testnet deployment walkthrough

### Prerequisites

**Required tools:**
- Rust and Cargo (for building)
- Stellar CLI (`stellar`) - [installation guide](https://developers.stellar.org/docs/soroban/install)
- Make (optional, for using the Makefile)

**Required accounts:**
- Deployer account with XLM on testnet (for paying deployment fees)
- Admin address (public Stellar address that will control the contract)
- Ed25519 signing key (private key used to sign mint payloads)

**⚠️ Security note:** The admin address and Ed25519 signing key are separate:
- **Admin address**: Public Stellar address stored on-chain for authorization
- **Ed25519 signing key**: Private key used to sign mint payloads (never stored on-chain)
- Keep the Ed25519 private key secure - it can authorize unlimited mints

### Step 1: Build the contract

```bash
# Using Make
make build

# Or using cargo directly
cargo build --release --target wasm32-unknown-unknown
```

This produces the WASM file at `target/wasm32-unknown-unknown/release/stellar_wrap_contract.wasm`.

### Step 2: Deploy to testnet

Set your deployer secret key as an environment variable:

```bash
export STELLAR_DEPLOYER_SECRET="S..."
```

Deploy the contract:

```bash
# Using Make
make deploy-testnet

# Or using stellar CLI directly
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/stellar_wrap_contract.wasm \
  --network testnet \
  --source "$STELLAR_DEPLOYER_SECRET"
```

Save the contract ID output - you'll need it for initialization.

### Step 3: Initialize the contract

You need:
- `CONTRACT_ID`: From step 2
- `ADMIN_ADDRESS`: Your admin Stellar address (public)
- `ADMIN_PUBKEY`: The 32-byte public key of your Ed25519 signing key

To get your Ed25519 public key from your private signing key:

```bash
# If you have the private key in hex format
# This is a placeholder - use your actual Ed25519 key generation tool
# The public key is 32 bytes
```

Initialize the contract:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  --source "$STELLAR_DEPLOYER_SECRET" \
  -- initialize \
  --admin <ADMIN_ADDRESS> \
  --admin_pubkey <ADMIN_PUBKEY_HEX>
```

### Step 4: Mint your first wrap

You need to sign a payload with your Ed25519 signing key. The payload includes:
- Contract address
- User address (who will receive the wrap)
- Period (YYYYMM format)
- Archetype (symbol)
- Data hash (SHA-256 of your wrap data)

Example using a signing script (you'll need to implement this based on your Ed25519 library):

```bash
# 1. Prepare your data and hash it
echo '{"score":100,"level":"gold"}' > data.json
DATA_HASH=$(sha256sum data.json | cut -d' ' -f1)

# 2. Sign the payload with your Ed25519 private key
# (Use your preferred Ed25519 signing tool)
SIGNATURE=$(sign-payload \
  --contract <CONTRACT_ID> \
  --user <USER_ADDRESS> \
  --period 202401 \
  --archetype "arch" \
  --data_hash $DATA_HASH \
  --private-key <ED25519_PRIVATE_KEY>)

# 3. Mint the wrap
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  --source <USER_ADDRESS_SECRET> \
  -- mint_wrap \
  --user <USER_ADDRESS> \
  --period 202401 \
  --archetype "arch" \
  --data_hash $DATA_HASH \
  --signature $SIGNATURE
```

### Step 5: Verify the mint

Query the contract to verify the wrap was minted:

```bash
stellar contract read \
  --id <CONTRACT_ID> \
  --network testnet \
  -- get_wrap \
  --user <USER_ADDRESS> \
  --period 202401
```

### Upgrading an existing contract

To upgrade an existing contract instead of deploying fresh:

```bash
export CONTRACT_ID="<EXISTING_CONTRACT_ID>"
make deploy-testnet
```

This will upload the new WASM without creating a new contract instance.
## Upgrade compatibility

An upgrade replaces contract code while keeping storage, so any change to the
storage layout must ship as a numbered migration:

- `DataKey::MigrationVersion` stores the highest migration version applied (`0` before any migration).
- `migrate(version)` is admin-only and only accepts a version greater than the stored one, so a
  migration can never run twice — a replay panics with `MigrationAlreadyApplied` (#7).
- Additive changes (new `DataKey` variants, new methods) need no migration; changing or removing
  the shape of an existing key does, and the new code must bump the migration version.
- Call `migrate` in the same transaction batch as the upgrade, and verify with `migration_version()`.

Transfer support adds `WrapPeriods(Address)`, a bounded ownership index used to
recompute `get_latest_wrap` after a transfer. Fresh mints populate it
automatically. When upgrading a deployment that already has wrap records, the
admin must call `backfill_wrap_periods(user, periods)` once per existing owner
before those owners can mint or transfer. The method validates the count,
uniqueness, and existence of every supplied period before writing the index,
preventing a partial ownership index from being accepted.

## Development

The toolchain is pinned in `rust-toolchain.toml` (Rust 1.94.1 with the
`wasm32-unknown-unknown` target), so local, Docker, and CI builds match. With
`rustup` installed, the correct toolchain is selected automatically.

Run the test suite with:

```bash
cargo test
```

Build the WASM artifact with:

```bash
cargo build --release --target wasm32-unknown-unknown
```
