# Stellar Wrap Contract

Soroban contract for storing non-transferable Stellar Wrap records by wallet and reporting successful wrap mints through events.

## Contract layout

The contract is split into focused modules:

- `src/lib.rs`: contract type and module wiring
- `src/admin.rs`: initialization and admin updates
- `src/mint.rs`: period validation, signature verification, wrap minting, event emission
- `src/queries.rs`: read-only queries and metadata
- `src/errors.rs`: contract error codes
- `src/storage_types.rs`: storage keys and persisted record types

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

## Storage keys

- `DataKey::Admin`
- `DataKey::AdminPubKey`
- `DataKey::Wrap(Address, u64)`
- `DataKey::WrapCount(Address)`
- `DataKey::LatestPeriod(Address)`

## Public interface

### Write methods

- `initialize(e: Env, admin: Address, admin_pubkey: BytesN<32>)`
- `update_admin(e: Env, new_admin: Address)`
- `mint_wrap(e: Env, user: Address, period: u64, archetype: Symbol, data_hash: BytesN<32>, signature: BytesN<64>)`

### Read methods

- `get_wrap(e: Env, user: Address, period: u64) -> Option<WrapRecord>`
- `balance_of(e: Env, user: Address) -> i128`
- `verify_data(e: Env, user: Address, period: u64, data: Bytes) -> bool`
- `get_latest_wrap(e: Env, user: Address) -> Option<WrapRecord>`
- `get_admin(e: Env) -> Option<Address>`
- `name(e: Env) -> String`
- `symbol(e: Env) -> String`
- `decimals(e: Env) -> u32`

## Event schema

Successful wrap mints emit one event:

- Topic 0: `mint`
- Topic 1: `user` (`Address`)
- Topic 2: `period` (`u64`, `YYYYMM`)
- Data: `archetype` (`Symbol`)

Properties relevant to indexers:

- the event is emitted only after signature verification and storage writes succeed
- duplicate `(user, period)` mints are rejected, so one event equals one successful new wrap
- `period` is always a validated `YYYYMM` value

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

## Local Development Quickstart

### Prerequisites

- **Rust** – install via [rustup](https://rustup.rs/). The project targets a recent stable toolchain.
- **wasm32 target** – add the WebAssembly compilation target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- **Stellar CLI** (recommended) – install from the [Stellar soroban-cli releases](https://github.com/stellar/stellar-cli/releases) or via `cargo`:
  ```bash
  cargo install stellar-cli
  ```
  Alternatively, install the legacy **Soroban CLI**:
  ```bash
  cargo install soroban-cli
  ```

### Common commands

| Action | Command |
|---|---|
| Format | `cargo fmt` |
| Format check (CI) | `cargo fmt --check` or `make fmt-check` |
| Lint | `cargo clippy -- -D warnings` or `make lint` |
| Test | `cargo test` or `make test` |
| Release build (WASM) | `cargo build --release --target wasm32-unknown-unknown` or `make build` |
| Deploy to testnet | `make deploy-testnet` |
| Docker reproducible build | `make docker-build` or `docker build -t stellar-wrap-contract .` |

See the `Makefile` for the full list of targets (`make help`).

### Troubleshooting

**"target `wasm32-unknown-unknown` not installed"**
```bash
rustup target add wasm32-unknown-unknown
```

**SDK / toolchain mismatch errors** (e.g. `package \`soroban-sdk\` cannot be built because it requires a different Rust version`)

The Soroban SDK often tracks Rust nightly or a specific stable release. If you see version conflicts:
- Verify your Rust version matches what the lockfile expects:
  ```bash
  rustup show
  rustup update stable
  ```
- If the SDK pins a nightly, install and use it:
  ```bash
  rustup install nightly-YYYY-MM-DD
  rustup target add wasm32-unknown-unknown --toolchain nightly-YYYY-MM-DD
  cargo +nightly-YYYY-MM-DD build --release --target wasm32-unknown-unknown
  ```
- Clean stale artifacts before switching toolchains:
  ```bash
  cargo clean
  ```

**WASM build fails with link errors**
Ensure `wasm32-unknown-unknown` is the active target and no host-specific native dependencies leak in. The `Dockerfile` provides a fully isolated environment for reproducible WASM builds.
