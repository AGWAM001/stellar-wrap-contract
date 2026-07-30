# Architecture Decision: Upgradeable Proxy Pattern vs Native Upgrades

## Context
Issue #522 requested the refactoring of the contract to use the "upgradeable proxy pattern for seamless versioning". This pattern is common in EVM-based blockchains (e.g., EIP-1967 transparent proxies or UUPS) where a proxy contract delegates calls to a logic contract using `delegatecall` while maintaining storage in the proxy's context.

## Analysis of Stellar's State Model
In Soroban (Stellar's smart contract platform), the execution and storage model differs significantly from EVM:
1. **No `delegatecall` equivalent:** Soroban does not support executing another contract's logic within the current contract's storage context.
2. **Storage tied to Contract ID:** If a "Proxy" contract uses `env.invoke_contract()` to forward calls to an "Implementation" contract, the execution context switches. The Implementation contract will read and write to its *own* storage, not the Proxy's storage.
3. **Loss of State on Upgrade:** In a proxy setup, upgrading the implementation would require deploying a new logic contract (with a new Contract ID). Because storage is tied to the Contract ID, the new logic contract would have empty storage. State migration would be prohibitively expensive and complex.

## Decision
Implementing an EVM-style upgradeable proxy pattern is an **anti-pattern in Soroban** and would break the contract's ability to maintain continuous state across version upgrades.

Instead, Soroban provides a **native, secure upgrade mechanism**:
`env.deployer().update_current_contract_wasm(new_wasm_hash)`

This native capability is already fully implemented in `src/admin.rs` (`upgrade` function). It allows the contract logic (WASM) to be updated in-place while retaining the identical Contract ID and all existing persistent/instance storage.

## Conclusion
No Rust code refactoring was performed to introduce a Proxy struct, as doing so would compromise the contract's state model. The existing native upgrade pattern (`admin::upgrade`) is the correct and canonical architecture for seamless versioning in Soroban.
