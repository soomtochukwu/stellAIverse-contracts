use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Pool};

/* ---------------- ADMIN ---------------- */

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .expect("Contract not initialized")
}

pub fn require_admin(env: &Env, caller: &Address) {
    let admin = get_admin(env);
    if caller != &admin {
        panic!("Unauthorized: caller is not admin");
    }
}

/* ---------------- POOL COUNTER ---------------- */

pub fn get_pool_counter(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::PoolCounter)
        .unwrap_or(0)
}

pub fn set_pool_counter(env: &Env, counter: u64) {
    env.storage()
        .instance()
        .set(&DataKey::PoolCounter, &counter);
}

/* ---------------- POOL DATA ---------------- */

pub fn set_pool(env: &Env, pool: &Pool) {
    env.storage()
        .persistent()
        .set(&DataKey::Pool(pool.pool_id), pool);
}

pub fn get_pool(env: &Env, pool_id: u64) -> Pool {
    env.storage()
        .persistent()
        .get(&DataKey::Pool(pool_id))
        .expect("Pool not found")
}

/* ---------------- LP BALANCES ---------------- */

pub fn get_lp_balance(env: &Env, pool_id: u64, provider: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::LpBalance(pool_id, provider.clone()))
        .unwrap_or(0)
}

pub fn set_lp_balance(env: &Env, pool_id: u64, provider: &Address, amount: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::LpBalance(pool_id, provider.clone()), &amount);
}
