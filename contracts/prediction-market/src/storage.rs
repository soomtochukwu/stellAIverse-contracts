use soroban_sdk::{Env, Symbol};
use crate::types::Market;

pub const MARKET_COUNTER_KEY: &str = "pm_market_ctr";

pub fn get_counter(env: &Env, key: &str) -> u64 {
    env.storage().instance().get::<_, u64>(&Symbol::new(env, key)).unwrap_or(0)
}

pub fn increment_counter(env: &Env, key: &str) -> u64 {
    let next = get_counter(env, key).saturating_add(1);
    env.storage().instance().set(&Symbol::new(env, key), &next);
    next
}

pub fn store_market(env: &Env, m: &Market) {
    let key = (Symbol::new(env, "pm_market"), m.market_id);
    env.storage().persistent().set(&key, m);
}

pub fn get_market(env: &Env, market_id: u64) -> Option<Market> {
    let key = (Symbol::new(env, "pm_market"), market_id);
    env.storage().persistent().get(&key)
}
