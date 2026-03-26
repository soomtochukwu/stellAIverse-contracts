#![no_std]

mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, Address, Bytes, Env, Symbol, Vec};

use types::*;

#[contract]
pub struct LifecycleManager;

#[contractimpl]
impl LifecycleManager {
    /// Initialize the lifecycle manager with an admin and default TTL config.
    pub fn init_contract(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Contract already initialized");
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);

        let config = TtlConfig {
            active_ttl: DEFAULT_ACTIVE_TTL,
            historical_ttl: DEFAULT_HISTORICAL_TTL,
            archived_ttl: DEFAULT_ARCHIVED_TTL,
        };
        env.storage().instance().set(&DataKey::TtlConfig, &config);
    }

    /// Extend the TTL of a persistent storage entry based on its lifecycle type.
    pub fn extend_ttl(env: Env, key: Symbol, lifecycle_type: DataLifecycle) {
        let config = Self::get_ttl_config(env.clone());

        let ttl = match lifecycle_type {
            DataLifecycle::Active => config.active_ttl,
            DataLifecycle::Historical => config.historical_ttl,
            DataLifecycle::Archived => config.archived_ttl,
        };

        let threshold = ttl / TTL_THRESHOLD_DIVISOR;

        if env.storage().persistent().has(&key) {
            env.storage().persistent().extend_ttl(&key, threshold, ttl);
        }

        env.events()
            .publish((Symbol::new(&env, "TtlExtended"),), (&key, ttl));
    }

    /// Batch cleanup of expired entries. Removes entries that no longer exist
    /// in persistent storage (already expired) from the tracked entry state map.
    pub fn cleanup_expired(env: Env, admin: Address, keys: Vec<Symbol>) {
        admin.require_auth();
        require_admin(&env, &admin);

        let mut removed_count: u32 = 0;

        for key in keys.iter() {
            // If the entry no longer exists in persistent storage, it has expired.
            if !env.storage().persistent().has(&key) {
                // Clean up tracked lifecycle state.
                let state_key = DataKey::EntryState(key.clone());
                if env.storage().persistent().has(&state_key) {
                    env.storage().persistent().remove(&state_key);
                }
                removed_count += 1;
            }
        }

        env.events()
            .publish((Symbol::new(&env, "CleanupCompleted"),), removed_count);
    }

    /// Archive an entry: move data from the active key to an archived key
    /// and apply archived TTL. The entry's lifecycle state is updated.
    pub fn archive_entry(env: Env, admin: Address, key: Symbol, archived_key: Symbol) {
        admin.require_auth();
        require_admin(&env, &admin);

        assert!(env.storage().persistent().has(&key), "Entry not found");

        // Move the data.
        let data: Bytes = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Entry not found");

        env.storage().persistent().set(&archived_key, &data);
        env.storage().persistent().remove(&key);

        // Apply archived TTL.
        let config = Self::get_ttl_config(env.clone());
        let threshold = config.archived_ttl / TTL_THRESHOLD_DIVISOR;
        env.storage()
            .persistent()
            .extend_ttl(&archived_key, threshold, config.archived_ttl);

        // Track lifecycle state.
        env.storage().persistent().set(
            &DataKey::EntryState(archived_key.clone()),
            &DataLifecycle::Archived,
        );

        // Remove old state tracking.
        let old_state_key = DataKey::EntryState(key.clone());
        if env.storage().persistent().has(&old_state_key) {
            env.storage().persistent().remove(&old_state_key);
        }

        env.events()
            .publish((Symbol::new(&env, "EntryArchived"),), (&key, &archived_key));
    }

    /// Get the current TTL configuration.
    pub fn get_ttl_config(env: Env) -> TtlConfig {
        env.storage()
            .instance()
            .get(&DataKey::TtlConfig)
            .expect("Contract not initialized")
    }

    /// Admin-only: update the TTL configuration.
    pub fn set_ttl_config(env: Env, admin: Address, config: TtlConfig) {
        admin.require_auth();
        require_admin(&env, &admin);

        assert!(config.active_ttl > 0, "Active TTL must be positive");
        assert!(config.historical_ttl > 0, "Historical TTL must be positive");
        assert!(config.archived_ttl > 0, "Archived TTL must be positive");
        assert!(
            config.active_ttl > config.historical_ttl,
            "Active TTL must exceed historical"
        );
        assert!(
            config.historical_ttl > config.archived_ttl,
            "Historical TTL must exceed archived"
        );

        env.storage().instance().set(&DataKey::TtlConfig, &config);

        env.events().publish(
            (Symbol::new(&env, "TtlConfigUpdated"),),
            (
                &config.active_ttl,
                &config.historical_ttl,
                &config.archived_ttl,
            ),
        );
    }

    /// Batch extend TTL for multiple keys with the same lifecycle type.
    pub fn batch_extend(env: Env, keys: Vec<Symbol>, lifecycle_type: DataLifecycle) {
        for key in keys.iter() {
            Self::extend_ttl(env.clone(), key, lifecycle_type.clone());
        }
    }

    /// Get the lifecycle state of a tracked entry.
    pub fn get_entry_state(env: Env, key: Symbol) -> DataLifecycle {
        env.storage()
            .persistent()
            .get(&DataKey::EntryState(key))
            .unwrap_or(DataLifecycle::Active)
    }
}

/* ─── Internal helpers ─────────────────────────────────────────── */

fn require_admin(env: &Env, caller: &Address) {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .expect("Contract not initialized");
    if caller != &admin {
        panic!("Unauthorized: caller is not admin");
    }
}
