#![allow(unused_imports)]
use soroban_sdk::{contracttype, Address, String};

#[contracttype]
#[derive(Clone, Debug)]
pub struct Market {
    pub market_id: u64,
    pub creator: Address,
    pub description: String,
    pub status: MarketStatus,
    pub outcome_a_reserve: i128,
    pub outcome_b_reserve: i128,
    pub total_liquidity: i128,
    pub created_at: u64,
    pub resolved_outcome: Outcome,
}

#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Outcome {
    Unresolved = 0,
    A = 1,
    B = 2,
}

#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum MarketStatus {
    Open = 0,
    Resolved = 1,
    Disputed = 2,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct LiquidityPosition {
    pub provider: Address,
    pub market_id: u64,
    pub shares: u128,
    pub entry_a: i128,
    pub entry_b: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BetPosition {
    pub bettor: Address,
    pub market_id: u64,
    pub outcome: Outcome,
    pub tokens: u128,
    pub amount_paid: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Dispute {
    pub dispute_id: u64,
    pub market_id: u64,
    pub challenger: Address,
    pub bond: i128,
    pub votes_for: u128,
    pub votes_against: u128,
    pub deadline: u64,
    pub reason: String,
}
