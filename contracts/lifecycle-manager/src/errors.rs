use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum LifecycleError {
    /// Contract not initialized.
    NotInitialized = 1,
    /// Caller is not admin.
    Unauthorized = 2,
    /// Entry not found in storage.
    EntryNotFound = 3,
    /// Invalid TTL configuration values.
    InvalidConfig = 4,
    /// Entry is already in the target lifecycle state.
    AlreadyInState = 5,
}
