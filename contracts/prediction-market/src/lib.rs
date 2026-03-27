#![no_std]

mod storage;
mod types;

use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, String};
use types::*;
use storage::*;

#[contract]
pub struct PredictionMarket;

#[contractimpl]
impl PredictionMarket {
    pub fn create_market(env: Env, creator: Address, market_id: u64, description: String) {
        creator.require_auth();
        // minimal market creation: store market with two outcome reserves = 0
        let now = env.ledger().timestamp();
        let m = Market {
            market_id,
            creator: creator.clone(),
            description: description.clone(),
            status: MarketStatus::Open,
            outcome_a_reserve: 0i128,
            outcome_b_reserve: 0i128,
            total_liquidity: 0i128,
            created_at: now,
            resolved_outcome: Outcome::Unresolved,
        };
        store_market(&env, &m);
        env.events().publish((Symbol::new(&env, "market_created"),), (market_id,));
    }

    pub fn provide_liquidity(env: Env, provider: Address, market_id: u64, amount_a: i128, amount_b: i128) {
        provider.require_auth();
        let mut m = match get_market(&env, market_id) {
            Some(x) => x,
            None => panic!("market not found"),
        };
        // naive liquidity add
        m.outcome_a_reserve = m.outcome_a_reserve.saturating_add(amount_a);
        m.outcome_b_reserve = m.outcome_b_reserve.saturating_add(amount_b);
        m.total_liquidity = m.total_liquidity.saturating_add(amount_a.saturating_add(amount_b));
        store_market(&env, &m);
        env.events().publish((Symbol::new(&env, "liquidity_added"),), (market_id,));
    }

    pub fn place_bet(env: Env, bettor: Address, market_id: u64, outcome: Outcome, amount: i128) {
        bettor.require_auth();
        let mut m = match get_market(&env, market_id) {
            Some(x) => x,
            None => panic!("market not found"),
        };
        // simple routing: add amount to chosen outcome reserve
        match outcome {
            Outcome::A => m.outcome_a_reserve = m.outcome_a_reserve.saturating_add(amount),
            Outcome::B => m.outcome_b_reserve = m.outcome_b_reserve.saturating_add(amount),
            _ => panic!("invalid outcome"),
        }
        store_market(&env, &m);
        env.events().publish((Symbol::new(&env, "bet_placed"),), (market_id,));
    }

    pub fn resolve_market(env: Env, caller: Address, market_id: u64, winning: Outcome) {
        caller.require_auth();
        // for now require admin via stellai_lib admin verify
        if stellai_lib::admin::verify_admin(&env, &caller).is_err() {
            panic!("unauthorized");
        }
        let mut m = match get_market(&env, market_id) {
            Some(x) => x,
            None => panic!("market not found"),
        };
        m.status = MarketStatus::Resolved;
        m.resolved_outcome = winning;
        store_market(&env, &m);
        env.events().publish((Symbol::new(&env, "market_resolved"),), (market_id, winning as u32));
    }
}
