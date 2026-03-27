use soroban_sdk::{contracttype, Symbol};

/// Lifecycle states for ledger entries.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataLifecycle {
    /// Active entries: ongoing agents, active listings (~1 year TTL).
    Active,
    /// Historical entries: completed requests, past transactions (~6 months TTL).
    Historical,
    /// Archived entries: compressed/archived data (~1 month TTL).
    Archived,
}

/// TTL configuration (in ledger counts).
#[contracttype]
#[derive(Clone, Debug)]
pub struct TtlConfig {
    /// TTL for active entries (default: 52560 ledgers, ~1 year).
    pub active_ttl: u32,
    /// TTL for historical entries (default: 26280 ledgers, ~6 months).
    pub historical_ttl: u32,
    /// TTL for archived entries (default: 5256 ledgers, ~1 month).
    pub archived_ttl: u32,
}

/// Storage keys for the lifecycle manager.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Admin address.
    Admin,
    /// TTL configuration.
    TtlConfig,
    /// Track lifecycle state of a managed entry.
    EntryState(Symbol),
}

/// Default TTL values.
pub const DEFAULT_ACTIVE_TTL: u32 = 52560;
pub const DEFAULT_HISTORICAL_TTL: u32 = 26280;
pub const DEFAULT_ARCHIVED_TTL: u32 = 5256;

/// Bump threshold: extend when TTL falls below half the target.
pub const TTL_THRESHOLD_DIVISOR: u32 = 2;
