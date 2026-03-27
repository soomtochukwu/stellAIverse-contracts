#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, Address, PredictionMarketClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register(PredictionMarket, ());
    let client = PredictionMarketClient::new(&env, &contract_id);
    (env, admin, client)
}

#[test]
fn test_create_market() {
    let (env, _admin, client) = setup();
    let creator = Address::generate(&env);
    env.mock_all_auths();
    client.create_market(&creator, &1u64, &String::from_slice(&env, "Test market"));
    let m = client.get_market(&1u64);
    assert!(m.is_some());
}
