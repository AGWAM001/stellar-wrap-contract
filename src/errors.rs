use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    WrapAlreadyExists = 4,
    InvalidSignature = 5,
    InvalidPeriod = 6,
    MigrationAlreadyApplied = 7,
    WrapNotFound = 8,
    InvalidTransfer = 9,
    TransferFeeNotConfigured = 10,
    InvalidFee = 11,
    TransferInProgress = 12,
    StorageInvariantViolation = 13,
}
