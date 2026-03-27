use crate::types::{BetPosition, Dispute, LiquidityPosition, Market};
use soroban_sdk::{Env, Symbol};

pub const MARKET_COUNTER_KEY: &str = "pm_market_ctr";
pub const LIQUIDITY_COUNTER_KEY: &str = "pm_liq_ctr";
pub const DISPUTE_COUNTER_KEY: &str = "pm_disp_ctr";

pub fn get_counter(env: &Env, key: &str) -> u64 {
    env.storage()
        .instance()
        .get::<_, u64>(&Symbol::new(env, key))
        .unwrap_or(0)
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

pub fn store_liquidity_position(env: &Env, pos: &LiquidityPosition) {
    let key = (
        Symbol::new(env, "pm_liq_pos"),
        pos.provider.clone(),
        pos.market_id,
    );
    env.storage().persistent().set(&key, pos);
}

pub fn get_liquidity_position(
    env: &Env,
    provider: &soroban_sdk::Address,
    market_id: u64,
) -> Option<LiquidityPosition> {
    let key = (Symbol::new(env, "pm_liq_pos"), provider, market_id);
    env.storage().persistent().get(&key)
}

pub fn store_bet_position(env: &Env, pos: &BetPosition) {
    let key = (
        Symbol::new(env, "pm_bet_pos"),
        pos.bettor.clone(),
        pos.market_id,
    );
    env.storage().persistent().set(&key, pos);
}

pub fn get_bet_position(
    env: &Env,
    bettor: &soroban_sdk::Address,
    market_id: u64,
) -> Option<BetPosition> {
    let key = (Symbol::new(env, "pm_bet_pos"), bettor, market_id);
    env.storage().persistent().get(&key)
}

pub fn store_dispute(env: &Env, dispute: &Dispute) {
    let key = (Symbol::new(env, "pm_dispute"), dispute.dispute_id);
    env.storage().persistent().set(&key, dispute);
}

pub fn get_dispute(env: &Env, dispute_id: u64) -> Option<Dispute> {
    let key = (Symbol::new(env, "pm_dispute"), dispute_id);
    env.storage().persistent().get(&key)
}
