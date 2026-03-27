#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as TestAddress, Env as TestEnv};
use soroban_sdk::{Address, Env};

fn setup() -> (Env, Address, PredictionMarketClient<'static>) {
    let env = TestEnv::default();
    let admin = Address::generate(&env);
    let contract_id = env.register(PredictionMarket, ());
    let client = PredictionMarketClient::new(&env, &contract_id);
    (env, admin, client)
}

#[test]
fn test_create_market() {
    let (env, _admin, client) = setup();
    let creator = Address::generate(&env);
    client.create_market(&creator, &1u64, &String::from_str(&env, "Test market"));
    let m = client.get_market(&1u64);
    assert!(m.is_some());
}

#[test]
fn test_add_liquidity() {
    let (env, _admin, client) = setup();
    let creator = Address::generate(&env);
    let provider = Address::generate(&env);

    // Create market
    client.create_market(&creator, &1u64, &String::from_str(&env, "Test market"));

    // Add liquidity
    let shares = client.add_liquidity(&provider, &1u64, &1000i128);
    assert!(shares > 0);

    // Check market state
    let market = client.get_market(&1u64).unwrap();
    assert_eq!(market.total_liquidity, 1000);
    assert_eq!(market.outcome_a_reserve, 500);
    assert_eq!(market.outcome_b_reserve, 500);
}

#[test]
fn test_remove_liquidity() {
    let (env, _admin, client) = setup();
    let creator = Address::generate(&env);
    let provider = Address::generate(&env);

    // Create market and add liquidity
    client.create_market(&creator, &1u64, &String::from_str(&env, "Test market"));
    let shares = client.add_liquidity(&provider, &1u64, &1000i128);

    // Remove liquidity
    let (amount_a, amount_b) = client.remove_liquidity(&provider, &1u64, &shares);
    assert!(amount_a > 0);
    assert!(amount_b > 0);
}

#[test]
fn test_get_price() {
    let (env, _admin, client) = setup();
    let creator = Address::generate(&env);
    let provider = Address::generate(&env);

    // Create market and add liquidity
    client.create_market(&creator, &1u64, &String::from_str(&env, "Test market"));
    client.add_liquidity(&provider, &1u64, &1000i128);

    // Get prices
    let price_a = client.get_price(&1u64, &super::Outcome::A);
    let price_b = client.get_price(&1u64, &super::Outcome::B);

    // Prices should be around 5000 bps (50%) for balanced liquidity
    assert_eq!(price_a, 5000);
    assert_eq!(price_b, 5000);
}

#[test]
fn test_place_bet_amm() {
    let (env, _admin, client) = setup();
    let creator = Address::generate(&env);
    let provider = Address::generate(&env);
    let bettor = Address::generate(&env);

    // Create market and add liquidity
    client.create_market(&creator, &1u64, &String::from_str(&env, "Test market"));
    client.add_liquidity(&provider, &1u64, &1000i128);

    // Place bet
    let tokens = client.place_bet_amm(&bettor, &1u64, &super::Outcome::A, &100i128);
    assert!(tokens > 0);
}

#[test]
fn test_claim_winnings() {
    let (env, admin, client) = setup();
    let creator = Address::generate(&env);
    let provider = Address::generate(&env);
    let bettor = Address::generate(&env);

    // Create market and add liquidity
    client.create_market(&creator, &1u64, &String::from_str(&env, "Test market"));
    client.add_liquidity(&provider, &1u64, &1000i128);

    // Place bet
    client.place_bet_amm(&bettor, &1u64, &super::Outcome::A, &100i128);

    // Resolve market
    client.resolve_market(&admin, &1u64, &super::Outcome::A);

    // Claim winnings
    let winnings = client.claim_winnings(&bettor, &1u64);
    assert!(winnings > 0);
}

#[test]
fn test_dispute_outcome() {
    let (env, admin, client) = setup();
    let creator = Address::generate(&env);
    let challenger = Address::generate(&env);

    // Create and resolve market
    client.create_market(&creator, &1u64, &String::from_str(&env, "Test market"));
    client.resolve_market(&admin, &1u64, &super::Outcome::A);

    // Create dispute
    let dispute_id = client.dispute_outcome(
        &challenger,
        &1u64,
        &100i128,
        &String::from_str(&env, "Incorrect resolution"),
    );
    assert!(dispute_id > 0);
}

#[test]
fn test_vote_on_dispute() {
    let (env, admin, client) = setup();
    let creator = Address::generate(&env);
    let challenger = Address::generate(&env);
    let voter = Address::generate(&env);

    // Create, resolve, and dispute market
    client.create_market(&creator, &1u64, &String::from_str(&env, "Test market"));
    client.resolve_market(&admin, &1u64, &super::Outcome::A);
    let dispute_id = client.dispute_outcome(
        &challenger,
        &1u64,
        &100i128,
        &String::from_str(&env, "Test dispute"),
    );

    // Vote on dispute
    client.vote_on_dispute(&voter, &dispute_id, &true);
}

#[test]
fn test_create_agent_market() {
    let (env, _admin, client) = setup();
    let agent = Address::generate(&env);

    // Create agent market with initial liquidity
    client.create_agent_market(
        &agent,
        &1u64,
        &String::from_str(&env, "Agent market"),
        &1000i128,
    );

    // Verify market was created
    let market = client.get_market(&1u64).unwrap();
    assert_eq!(market.creator, agent);
    assert_eq!(market.total_liquidity, 1000);
}

#[test]
fn test_place_bet_reputation_weighted() {
    let (env, _admin, client) = setup();
    let creator = Address::generate(&env);
    let provider = Address::generate(&env);
    let bettor = Address::generate(&env);

    // Create market and add liquidity
    client.create_market(&creator, &1u64, &String::from_str(&env, "Test market"));
    client.add_liquidity(&provider, &1u64, &1000i128);

    // Place reputation-weighted bet
    let tokens = client.place_bet_reputation_weighted(&bettor, &1u64, &super::Outcome::A, &100i128);
    assert!(tokens > 0);
}

#[test]
fn test_market_lifecycle() {
    let (env, admin, client) = setup();
    let creator = Address::generate(&env);
    let provider = Address::generate(&env);
    let bettor_a = Address::generate(&env);
    let bettor_b = Address::generate(&env);

    // 1. Create market
    client.create_market(&creator, &1u64, &String::from_str(&env, "Test market"));

    // 2. Add liquidity
    let shares = client.add_liquidity(&provider, &1u64, &1000i128);
    assert!(shares > 0);

    // 3. Place bets on both outcomes
    let tokens_a = client.place_bet_amm(&bettor_a, &1u64, &super::Outcome::A, &100i128);
    let tokens_b = client.place_bet_amm(&bettor_b, &1u64, &super::Outcome::B, &100i128);
    assert!(tokens_a > 0);
    assert!(tokens_b > 0);

    // 4. Resolve market
    client.resolve_market(&admin, &1u64, &super::Outcome::A);

    // 5. Claim winnings (bettor A should win)
    let winnings_a = client.claim_winnings(&bettor_a, &1u64);
    let winnings_b = client.claim_winnings(&bettor_b, &1u64);

    assert!(winnings_a > 0);
    assert_eq!(winnings_b, 0); // Loser gets nothing
}

#[test]
fn test_liquidity_provider_rewards() {
    let (env, admin, client) = setup();
    let creator = Address::generate(&env);
    let provider = Address::generate(&env);
    let bettor = Address::generate(&env);

    // Create market and add liquidity
    client.create_market(&creator, &1u64, &String::from_str(&env, "Test market"));
    client.add_liquidity(&provider, &1u64, &1000i128);

    // Place bet to generate fees
    client.place_bet_amm(&bettor, &1u64, &super::Outcome::A, &100i128);

    // Resolve market
    client.resolve_market(&admin, &1u64, &super::Outcome::A);

    // Remove liquidity (should include fees)
    let (amount_a, amount_b) = client.remove_liquidity(&provider, &1u64, &1000000u128);
    assert!(amount_a > 500); // Should get more than initial due to fees
}
