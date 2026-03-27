#![cfg(test)]

use super::*;
use soroban_sdk::{Address, Env, String};

#[test]
fn test_contract_compiles() {
    // This test verifies that the contract compiles correctly
    // and the basic types and functions are available

    let env = Env::default();

    // Test that we can create basic types
    let market = Market {
        market_id: 1,
        creator: Address::generate(&env),
        description: String::from_str(&env, "Test"),
        status: MarketStatus::Open,
        outcome_a_reserve: 100,
        outcome_b_reserve: 100,
        total_liquidity: 200,
        created_at: 12345,
        resolved_outcome: Outcome::Unresolved,
    };

    // Test that we can create other types
    let _position = LiquidityPosition {
        provider: Address::generate(&env),
        market_id: 1,
        shares: 1000,
        entry_a: 50,
        entry_b: 50,
    };

    let _bet = BetPosition {
        bettor: Address::generate(&env),
        market_id: 1,
        outcome: Outcome::A,
        tokens: 100,
        amount_paid: 100,
    };

    // Test that enums work
    let _outcome = Outcome::A;
    let _status = MarketStatus::Resolved;
    let _reason = ReputationReason::Execution;

    // If we get here, everything compiles correctly
    assert_eq!(market.market_id, 1);
}
