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
    // Staking errors
    StakeTooLow = 15,
    StakeNotFound = 16,
    StakeCooldownActive = 17,
    StakeNotUnstaking = 18,
    StakeCooldownNotElapsed = 19,
    InvalidStakeConfig = 20,
    StakeArithmeticOverflow = 21,
}
