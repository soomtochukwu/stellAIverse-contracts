use soroban_sdk::{contract, contractimpl, Env, Symbol};
use crate::types::DataLifecycle;

#[contract]
pub struct LifecycleManager;

#[contractimpl]
impl LifecycleManager {
    pub fn extend_ttl(env: Env, key: Symbol, lifecycle: DataLifecycle) {
        let ttl = match lifecycle {
            DataLifecycle::Active => ACTIVE_TTL,
            DataLifecycle::Historical => HISTORICAL_TTL,
            DataLifecycle::Archived => ARCHIVED_TTL,
        };
        env.storage().persistent().extend(&key, ttl);
    }

    pub fn cleanup_expired_evolution(env: Env, key: Symbol) {
        if env.storage().persistent().has(&key) {
            let ttl = env.storage().persistent().ttl(&key).unwrap_or(0);
            if ttl == 0 {
                env.storage().persistent().remove(&key);
            }
        }
    }

    pub fn archive_listing(env: Env, listing_key: Symbol, archived_key: Symbol) {
        if let Some(listing) = env.storage().persistent().get::<_, Bytes>(&listing_key) {
            env.storage().persistent().set(&archived_key, &listing);
            env.storage().persistent().remove(&listing_key);
            env.storage().persistent().extend(&archived_key, ARCHIVED_TTL);
        }
    }

    pub fn batch_extend(env: Env, keys: Vec<Symbol>, lifecycle: DataLifecycle) {
        for key in keys {
            Self::extend_ttl(env.clone(), key, lifecycle.clone());
        }
    }
}
