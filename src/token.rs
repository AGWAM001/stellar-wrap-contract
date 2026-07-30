//! Standard token interface trait for the Stellar Wrap Registry.
//!
//! Extracts the standard token interface methods (`name`, `symbol`, `decimals`,
//! `balance_of`) into a dedicated trait, reducing boilerplate in `lib.rs` and
//! promoting interface reuse.
//!
//! The trait is implemented directly in `lib.rs` with `#[contractimpl]` so
//! the Soroban macro generates client types in the correct crate scope.
//!
//! # Standard Interface Methods
//!
//! | Method       | Description                         |
//! |--------------|-------------------------------------|
//! | `name`       | Token display name (default: "Stellar Wrap Registry") |
//! | `symbol`     | Token ticker symbol (default: "WRAP") |
//! | `decimals`   | Number of decimals (always 0 for soulbound wraps) |
//! | `balance_of` | Number of active wraps for a user   |

use soroban_sdk::{Address, Env, String};

/// Standard token interface trait for `StellarWrapContract`.
///
/// Implemented in `lib.rs` via `#[contractimpl]` so these methods are
/// automatically exposed as contract functions. The implementations
/// delegate to the `queries` module for storage access.
pub trait TokenInterface {
    /// Returns the token name, either the admin-set override or the default.
    fn name(e: Env) -> String;

    /// Returns the token symbol, either the admin-set override or the default.
    fn symbol(e: Env) -> String;

    /// Returns the number of decimals (always 0 for this contract).
    fn decimals(e: Env) -> u32;

    /// Returns the number of active wrap records for the given user.
    fn balance_of(e: Env, user: Address) -> i128;
}
