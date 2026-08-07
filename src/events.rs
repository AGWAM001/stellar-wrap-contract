//! Strongly typed contract events.
//!
//! Defines typed event symbols and data payloads for mint operations,
//! replacing raw `symbol_short!()` strings with a Rust enum. Each
//! variant converts to its corresponding `Symbol` and back, making
//! event names strongly typed throughout the codebase.

use soroban_sdk::{contracttype, Address, Env, Symbol};

use crate::storage_types::WrapState;

/// Strongly typed event names for mint operations.
///
/// Replace inline `symbol_short!("mint")` / `symbol_short!("trans")`
/// calls with typed enum values, reducing the risk of typos and
/// improving discoverability.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MintEventType {
    Mint,
    Transition,
}

impl MintEventType {
    /// Convert this event type to a Soroban `Symbol`.
    pub fn to_symbol(self, e: &Env) -> Symbol {
        match self {
            MintEventType::Mint => Symbol::new(e, "mint"),
            MintEventType::Transition => Symbol::new(e, "trans"),
        }
    }
}

/// Strongly typed event data payloads for mint operations.
///
/// Used as the data argument in `e.events().publish()` to provide
/// type-safe event emission instead of raw values.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MintEventData {
    /// A wrap was successfully minted.
    Mint(Address, u64, Symbol),
    /// A wrap's lifecycle state was transitioned.
    Transition(Address, u64, WrapState),
}
