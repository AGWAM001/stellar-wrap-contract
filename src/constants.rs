/// Domain separator string prepended to every signed mint payload.
/// Binds all signatures to this specific protocol version so that
/// future protocol changes cannot accidentally reuse old signatures.
pub const DOMAIN_SEPARATOR: &str = "StellarWrap-v1";
