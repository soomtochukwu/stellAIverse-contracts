#![no_std]

mod storage;
mod types;

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Symbol};
use storage::*;
use types::*;

#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ReputationReason {
    Execution = 0,
    Marketplace = 1,
    Prediction = 2,
}

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
        env.events()
            .publish((Symbol::new(&env, "market_created"),), (market_id,));
    }

    /// Add liquidity to a market using AMM
    pub fn add_liquidity(env: Env, provider: Address, market_id: u64, amount: i128) -> u128 {
        provider.require_auth();
        let mut m = match get_market(&env, market_id) {
            Some(x) => x,
            None => panic!("market not found"),
        };

        if m.status != MarketStatus::Open {
            panic!("market not open for liquidity");
        }

        // Calculate proportional amounts for both outcomes
        let total_reserves = m.outcome_a_reserve.saturating_add(m.outcome_b_reserve);
        let amount_a = if total_reserves == 0 {
            amount / 2 // Initial liquidity: split evenly
        } else {
            amount
                .saturating_mul(m.outcome_a_reserve)
                .checked_div(total_reserves)
                .unwrap_or(0)
        };
        let amount_b = amount.saturating_sub(amount_a);

        // Update reserves
        m.outcome_a_reserve = m.outcome_a_reserve.saturating_add(amount_a);
        m.outcome_b_reserve = m.outcome_b_reserve.saturating_add(amount_b);
        m.total_liquidity = m.total_liquidity.saturating_add(amount);

        // Calculate liquidity shares
        let shares = if m.total_liquidity == amount {
            amount as u128 // First liquidity provider gets proportional shares
        } else {
            let prev_liquidity = m.total_liquidity.saturating_sub(amount);
            if prev_liquidity > 0 {
                (amount as u128).saturating_mul(1000000) / prev_liquidity as u128
            } else {
                amount as u128
            }
        };

        // Store liquidity position
        let mut pos =
            get_liquidity_position(&env, &provider, market_id).unwrap_or(LiquidityPosition {
                provider: provider.clone(),
                market_id,
                shares: 0,
                entry_a: 0,
                entry_b: 0,
            });
        pos.shares = pos.shares.saturating_add(shares);
        pos.entry_a = pos.entry_a.saturating_add(amount_a);
        pos.entry_b = pos.entry_b.saturating_add(amount_b);
        store_liquidity_position(&env, &pos);

        store_market(&env, &m);
        env.events()
            .publish((Symbol::new(&env, "liquidity_added"),), (market_id, shares));
        shares
    }

    /// Remove liquidity from a market
    pub fn remove_liquidity(
        env: Env,
        provider: Address,
        market_id: u64,
        shares: u128,
    ) -> (i128, i128) {
        provider.require_auth();
        let m = match get_market(&env, market_id) {
            Some(x) => x,
            None => panic!("market not found"),
        };

        let mut pos = match get_liquidity_position(&env, &provider, market_id) {
            Some(x) => x,
            None => panic!("no liquidity position"),
        };

        if pos.shares < shares {
            panic!("insufficient shares");
        }

        // Calculate proportional withdrawal
        let total_supply = m.total_liquidity;
        let withdraw_amount_a = shares
            .saturating_mul(m.outcome_a_reserve as u128)
            .checked_div(total_supply as u128)
            .unwrap_or(0) as i128;
        let withdraw_amount_b = shares
            .saturating_mul(m.outcome_b_reserve as u128)
            .checked_div(total_supply as u128)
            .unwrap_or(0) as i128;

        // Update position
        pos.shares = pos.shares.saturating_sub(shares);
        if pos.shares == 0 {
            // Remove position if no shares left
            let key = (Symbol::new(&env, "pm_liq_pos"), provider, market_id);
            env.storage().persistent().remove(&key);
        } else {
            store_liquidity_position(&env, &pos);
        }

        // Update market reserves
        let mut updated_m = m;
        updated_m.outcome_a_reserve = updated_m
            .outcome_a_reserve
            .saturating_sub(withdraw_amount_a);
        updated_m.outcome_b_reserve = updated_m
            .outcome_b_reserve
            .saturating_sub(withdraw_amount_b);
        updated_m.total_liquidity = updated_m
            .total_liquidity
            .saturating_sub(withdraw_amount_a.saturating_add(withdraw_amount_b));
        store_market(&env, &updated_m);

        env.events().publish(
            (Symbol::new(&env, "liquidity_removed"),),
            (market_id, shares),
        );
        (withdraw_amount_a, withdraw_amount_b)
    }

    /// Get current price for an outcome using AMM formula
    pub fn get_price(env: Env, market_id: u64, outcome: Outcome) -> i128 {
        let m = match get_market(&env, market_id) {
            Some(x) => x,
            None => panic!("market not found"),
        };

        let total_reserves = m.outcome_a_reserve.saturating_add(m.outcome_b_reserve);
        if total_reserves == 0 {
            return 5000; // Default 50% probability (5000 bps)
        }

        let outcome_reserves = match outcome {
            Outcome::A => m.outcome_a_reserve,
            Outcome::B => m.outcome_b_reserve,
            _ => panic!("invalid outcome"),
        };

        // Return probability in basis points (0-10000)
        outcome_reserves
            .saturating_mul(10000)
            .checked_div(total_reserves)
            .unwrap_or(0)
    }

    /// Place a bet using AMM pricing
    pub fn place_bet_amm(
        env: Env,
        bettor: Address,
        market_id: u64,
        outcome: Outcome,
        amount: i128,
    ) -> u128 {
        bettor.require_auth();
        let mut m = match get_market(&env, market_id) {
            Some(x) => x,
            None => panic!("market not found"),
        };

        if m.status != MarketStatus::Open {
            panic!("market not open for betting");
        }

        // Calculate tokens received based on AMM formula
        let (tokens_a, tokens_b) = match outcome {
            Outcome::A => {
                let tokens_out =
                    calculate_tokens_out(m.outcome_a_reserve, m.outcome_b_reserve, amount);
                (tokens_out, 0)
            }
            Outcome::B => {
                let tokens_out =
                    calculate_tokens_out(m.outcome_b_reserve, m.outcome_a_reserve, amount);
                (0, tokens_out)
            }
            _ => panic!("invalid outcome"),
        };

        // Update reserves
        m.outcome_a_reserve = m.outcome_a_reserve.saturating_add(amount);
        m.outcome_b_reserve = m.outcome_b_reserve.saturating_add(amount);

        // Store bet position
        let mut pos = get_bet_position(&env, &bettor, market_id).unwrap_or(BetPosition {
            bettor: bettor.clone(),
            market_id,
            outcome,
            tokens: 0,
            amount_paid: 0,
        });
        pos.tokens = pos
            .tokens
            .saturating_add(tokens_a.saturating_add(tokens_b) as u128);
        pos.amount_paid = pos.amount_paid.saturating_add(amount);
        store_bet_position(&env, &pos);

        store_market(&env, &m);
        env.events().publish(
            (Symbol::new(&env, "bet_placed_amm"),),
            (market_id, outcome as u32, tokens_a.saturating_add(tokens_b)),
        );
        tokens_a.saturating_add(tokens_b) as u128
    }

    /// Claim winnings after market resolution
    pub fn claim_winnings(env: Env, bettor: Address, market_id: u64) -> i128 {
        bettor.require_auth();
        let m = match get_market(&env, market_id) {
            Some(x) => x,
            None => panic!("market not found"),
        };

        if m.status != MarketStatus::Resolved {
            panic!("market not resolved");
        }

        let pos = match get_bet_position(&env, &bettor, market_id) {
            Some(x) => x,
            None => panic!("no bet position"),
        };

        // Calculate winnings based on pool reserves
        let winning_reserve = match m.resolved_outcome {
            Outcome::A => m.outcome_a_reserve,
            Outcome::B => m.outcome_b_reserve,
            _ => return 0,
        };

        let total_winning_tokens = winning_reserve;
        let winnings = if total_winning_tokens > 0 {
            pos.tokens
                .saturating_mul(total_winning_tokens as u128)
                .checked_div(1000000) // Adjust for precision
                .unwrap_or(0) as i128
        } else {
            0
        };

        // Remove bet position after claiming
        let key = (Symbol::new(&env, "pm_bet_pos"), bettor, market_id);
        env.storage().persistent().remove(&key);

        env.events().publish(
            (Symbol::new(&env, "winnings_claimed"),),
            (market_id, winnings),
        );
        winnings
    }

    pub fn place_bet(
        env: Env,
        bettor: Address,
        market_id: u64,
        outcome: Outcome,
        amount: i128,
    ) {
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
        env.events()
            .publish((Symbol::new(&env, "bet_placed"),), (market_id,));
    }

    /// Create a new dispute for a market outcome
    pub fn dispute_outcome(
        env: Env,
        challenger: Address,
        market_id: u64,
        bond: i128,
        reason: String,
    ) -> u64 {
        challenger.require_auth();
        let mut m = match get_market(&env, market_id) {
            Some(x) => x,
            None => panic!("market not found"),
        };

        if m.status != MarketStatus::Resolved {
            panic!("market must be resolved to dispute");
        }

            // Create dispute
        let dispute_id = increment_counter(&env, DISPUTE_COUNTER_KEY);
        let dispute = Dispute {
            dispute_id,
            market_id,
            challenger: challenger.clone(),
            bond,
            votes_for: 0,
            votes_against: 0,
                deadline: env.ledger().timestamp().saturating_add(7 * 24 * 60 * 60), // 7 days
            reason: reason.clone(),
        };

        store_dispute(&env, &dispute);

        // Update market status
        m.status = MarketStatus::Disputed;
        store_market(&env, &m);

            env.events().publish(
            (Symbol::new(&env, "dispute_created"),),
            (dispute_id, market_id),
        );
        dispute_id
    }

    /// Vote on a dispute
    pub fn vote_on_dispute(env: Env, voter: Address, dispute_id: u64, support: bool) {
        voter.require_auth();
        let mut dispute = match get_dispute(&env, dispute_id) {
            Some(x) => x,
            None => panic!("dispute not found"),
        };

        if env.ledger().timestamp() > dispute.deadline {
            panic!("voting period ended");
        }

            // In a real implementation, this would check voting power/staking
            // For now, each address gets 1 vote
            if support {
                dispute.votes_for = dispute.votes_for.saturating_add(1);
            } else {
                dispute.votes_against = dispute.votes_against.saturating_add(1);
            }

            store_dispute(&env, &dispute);
            env.events().publish(
                (Symbol::new(&env, "dispute_vote"),),
                (dispute_id, support as u32),
            );
        }

        /// Resolve a dispute (admin only)
        pub fn resolve_dispute(env: Env, admin: Address, dispute_id: u64, uphold_dispute: bool) {
            admin.require_auth();
            if stellai_lib::admin::verify_admin(&env, &admin).is_err() {
                panic!("unauthorized");
            }

            let dispute = match get_dispute(&env, dispute_id) {
                Some(x) => x,
                None => panic!("dispute not found"),
            };

            let mut m = match get_market(&env, dispute.market_id) {
                Some(x) => x,
                None => panic!("market not found"),
            };

            if uphold_dispute && dispute.votes_for > dispute.votes_against {
                // Dispute upheld - reverse the original resolution
                m.resolved_outcome = match m.resolved_outcome {
                    Outcome::A => Outcome::B,
                    Outcome::B => Outcome::A,
                    _ => Outcome::Unresolved,
                };

                // Return bond to challenger
                // In a real implementation, this would transfer the bond
            }

            // Return market to resolved state
            m.status = MarketStatus::Resolved;
            store_market(&env, &m);

            // Remove dispute
            let key = (Symbol::new(&env, "pm_dispute"), dispute_id);
            env.storage().persistent().remove(&key);

            env.events().publish(
                (Symbol::new(&env, "dispute_resolved"),),
                (dispute_id, uphold_dispute as u32),
            );
        }

        /// Resolve a market (admin only)
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
            env.events().publish(
                (Symbol::new(&env, "market_resolved"),),
                (market_id, winning as u32),
            );
        }

        /// Create a market as an agent with reputation requirements
        pub fn create_agent_market(
            env: Env,
            agent: Address,
            market_id: u64,
            description: String,
            initial_liquidity: i128,
        ) {
            agent.require_auth();

            // Check agent reputation (minimum threshold for market creation)
            let reputation = get_agent_reputation(&env, &agent);
            if reputation < 1000 {
                // Minimum reputation score
                panic!("insufficient reputation to create market");
            }

            // Create the market
            let now = env.ledger().timestamp();
            let m = Market {
                market_id,
                creator: agent.clone(),
                description: description.clone(),
                status: MarketStatus::Open,
                outcome_a_reserve: initial_liquidity / 2,
                outcome_b_reserve: initial_liquidity / 2,
                total_liquidity: initial_liquidity,
                created_at: now,
                resolved_outcome: Outcome::Unresolved,
            };

            // Store initial liquidity position for the agent
            let initial_shares = initial_liquidity as u128;
            let pos = LiquidityPosition {
                provider: agent.clone(),
                market_id,
                shares: initial_shares,
                entry_a: initial_liquidity / 2,
                entry_b: initial_liquidity / 2,
            };

            store_market(&env, &m);
            store_liquidity_position(&env, &pos);

            // Update agent reputation for creating market
            update_agent_reputation(&env, &agent, 50, ReputationReason::Marketplace);

            env.events().publish(
                (Symbol::new(&env, "agent_market_created"),),
                (market_id, agent),
            );
        }

        /// Place a bet with reputation weighting
        pub fn place_bet_reputation_weighted(
            env: Env,
            bettor: Address,
            market_id: u64,
            outcome: Outcome,
            amount: i128,
        ) -> u128 {
            bettor.require_auth();
            let mut m = match get_market(&env, market_id) {
                Some(x) => x,
                None => panic!("market not found"),
            };

            if m.status != MarketStatus::Open {
                panic!("market not open for betting");
            }

            // Get bettor reputation for weighting
            let reputation = get_agent_reputation(&env, &bettor);
            let weight_multiplier = if reputation > 0 {
                1 + (reputation / 1000) // 1x base + reputation bonus
            } else {
                1
            };

            // Apply reputation weighting to bet amount
            let weighted_amount = amount.saturating_mul(weight_multiplier);

            // Calculate tokens received using AMM formula with weighted amount
            let (tokens_a, tokens_b) = match outcome {
                Outcome::A => {
                    let tokens_out = calculate_tokens_out(
                        m.outcome_a_reserve,
                        m.outcome_b_reserve,
                        weighted_amount,
                    );
                    (tokens_out, 0)
                }
                Outcome::B => {
                    let tokens_out = calculate_tokens_out(
                        m.outcome_b_reserve,
                        m.outcome_a_reserve,
                        weighted_amount,
                    );
                    (0, tokens_out)
                }
                _ => panic!("invalid outcome"),
            };

            // Update reserves with actual amount (not weighted)
            m.outcome_a_reserve = m.outcome_a_reserve.saturating_add(amount);
            m.outcome_b_reserve = m.outcome_b_reserve.saturating_add(amount);

            // Store bet position with weighted tokens
            let mut pos = get_bet_position(&env, &bettor, market_id).unwrap_or(BetPosition {
                bettor: bettor.clone(),
                market_id,
                outcome,
                tokens: 0,
                amount_paid: 0,
            });
            pos.tokens = pos
                .tokens
                .saturating_add(tokens_a.saturating_add(tokens_b) as u128);
            pos.amount_paid = pos.amount_paid.saturating_add(amount);
            store_bet_position(&env, &pos);

            store_market(&env, &m);

            // Update bettor reputation for participating
            update_agent_reputation(&env, &bettor, 10, ReputationReason::Execution);

            env.events().publish(
                (Symbol::new(&env, "reputation_bet_placed"),),
                (
                    market_id,
                    outcome as u32,
                    tokens_a.saturating_add(tokens_b),
                    reputation,
                ),
            );
            tokens_a.saturating_add(tokens_b) as u128
        }
    }

    /// Helper function to calculate tokens out using AMM formula
    fn calculate_tokens_out(reserve_in: i128, reserve_out: i128, amount_in: i128) -> i128 {
        // Using constant product formula: x * y = k
        // tokens_out = (reserve_out * amount_in) / (reserve_in + amount_in)
        let new_reserve_in = reserve_in.saturating_add(amount_in);
        if new_reserve_in == 0 {
            return 0;
        }

        let k = reserve_in.saturating_mul(reserve_out);
        let new_reserve_out = k.checked_div(new_reserve_in).unwrap_or(0);
        reserve_out.saturating_sub(new_reserve_out)
    }

    /// Helper function to get agent reputation from metrics aggregator
    fn get_agent_reputation(_env: &Env, _agent: &Address) -> i128 {
        // In a real implementation, this would call the metrics aggregator contract
        // For now, return a default reputation score
        // This could be enhanced to actually query the reputation contract
        1000 // Default reputation score
    }

    /// Helper function to update agent reputation
    fn update_agent_reputation(env: &Env, agent: &Address, amount: i128, reason: ReputationReason) {
        // In a real implementation, this would call the metrics aggregator contract
        // to update the agent's reputation based on their prediction market activity
        // For now, we just emit an event
        env.events().publish(
            (Symbol::new(env, "reputation_updated"),),
            (agent, amount, reason as u32),
        );
    }
}
