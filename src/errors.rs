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
    InvalidStateTransition = 8,
    WrapNotFound = 9,
    NoAdminTransferProposal = 10,
    AdminTransferProposalExists = 11,
    Paused = 12,
    ArithmeticOverflow = 13,
    InvalidFeeParams = 14,
    /// No whitelist merkle root has been published yet.
    MerkleRootNotSet = 15,
    /// The supplied merkle proof does not prove membership in the whitelist.
    InvalidMerkleProof = 16,
    /// The timelock delay for this operation has not elapsed yet.
    TimelockNotReady = 17,
    /// No scheduled timelock operation exists for the given id.
    TimelockOperationNotFound = 18,
    /// An identical timelock operation is already scheduled.
    TimelockOperationExists = 19,
    /// The requested timelock delay is outside the allowed bounds.
    InvalidTimelockDelay = 20,
    /// The timelock is enabled, so this action must be scheduled and executed
    /// through the timelock controller instead of called directly.
    TimelockRequired = 21,
    /// The timelock has already been enabled and cannot be re-enabled.
    TimelockAlreadyEnabled = 22,
}
