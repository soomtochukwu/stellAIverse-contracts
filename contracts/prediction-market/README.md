# StellAIverse Prediction Market

An advanced prediction market contract with automated market making (AMM), reward distribution, dispute resolution, and agent ecosystem integration.

## Features

### Core Functionality
- **Market Creation**: Create binary prediction markets with custom descriptions
- **Automated Market Making (AMM)**: Constant product formula for price discovery
- **Liquidity Provision**: Add/remove liquidity with share-based rewards
- **Betting**: Place bets with AMM pricing and slippage protection
- **Reward Distribution**: Automatic winnings calculation and distribution

### Advanced Features
- **Dispute Resolution**: Multi-step dispute process with voting
- **Agent Integration**: Reputation-weighted betting and market creation
- **Oracle Bridge**: Integration with external oracle systems
- **Compliance**: Rate limiting, minimum liquidity, anti-manipulation

## Architecture

### Key Components

#### Market Structure
```rust
pub struct Market {
    pub market_id: u64,
    pub creator: Address,
    pub description: String,
    pub status: MarketStatus,          // Open, Resolved, Disputed
    pub outcome_a_reserve: i128,       // AMM reserve for outcome A
    pub outcome_b_reserve: i128,       // AMM reserve for outcome B
    pub total_liquidity: i128,         // Total liquidity in market
    pub created_at: u64,
    pub resolved_outcome: Outcome,      // Unresolved, A, B
}
```

#### Liquidity Positions
```rust
pub struct LiquidityPosition {
    pub provider: Address,
    pub market_id: u64,
    pub shares: u128,                  // Liquidity provider shares
    pub entry_a: i128,                 // Entry point for outcome A
    pub entry_b: i128,                 // Entry point for outcome B
}
```

#### Bet Positions
```rust
pub struct BetPosition {
    pub bettor: Address,
    pub market_id: u64,
    pub outcome: Outcome,               // A or B
    pub tokens: u128,                   // Outcome tokens held
    pub amount_paid: i128,              // Amount paid for tokens
}
```

#### Dispute System
```rust
pub struct Dispute {
    pub dispute_id: u64,
    pub market_id: u64,
    pub challenger: Address,
    pub bond: i128,                    // Bond posted by challenger
    pub votes_for: u128,               // Votes supporting dispute
    pub votes_against: u128,           // Votes against dispute
    pub deadline: u64,                 // Voting deadline
    pub reason: String,                // Dispute reason
}
```

## AMM Formula

The prediction market uses a constant product AMM formula:

```
x * y = k

Where:
- x = Reserve for outcome A
- y = Reserve for outcome B  
- k = Constant product

Price calculation:
price_A = (reserve_A / (reserve_A + reserve_B)) * 10000 bps
```

## Usage Examples

### Creating a Market
```rust
// Basic market creation
prediction_market.create_market(
    creator,
    1u64,                                    // market_id
    "Will AI achieve AGI by 2030?"           // description
);

// Agent market with initial liquidity
prediction_market.create_agent_market(
    agent,
    2u64,
    "Bitcoin price > $100k by end of year",
    1000i128                                 // initial_liquidity
);
```

### Providing Liquidity
```rust
// Add liquidity to existing market
let shares = prediction_market.add_liquidity(
    provider,
    1u64,                                    // market_id
    1000i128                                 // amount
);

// Remove liquidity
let (amount_a, amount_b) = prediction_market.remove_liquidity(
    provider,
    1u64,                                    // market_id
    shares                                   // shares_to_remove
);
```

### Placing Bets
```rust
// Standard AMM bet
let tokens = prediction_market.place_bet_amm(
    bettor,
    1u64,                                    // market_id
    Outcome::A,                              // outcome
    100i128                                  // amount
);

// Reputation-weighted bet (agents only)
let tokens = prediction_market.place_bet_reputation_weighted(
    agent,
    1u64,
    Outcome::B,
    100i128
);
```

### Market Resolution
```rust
// Resolve market (admin only)
prediction_market.resolve_market(
    admin,
    1u64,
    Outcome::A                               // winning_outcome
);

// Claim winnings
let winnings = prediction_market.claim_winnings(
    bettor,
    1u64                                     // market_id
);
```

### Dispute Process
```rust
// Create dispute
let dispute_id = prediction_market.dispute_outcome(
    challenger,
    1u64,
    100i128,                                 // bond
    "Oracle data appears incorrect"          // reason
);

// Vote on dispute
prediction_market.vote_on_dispute(
    voter,
    dispute_id,
    true                                     // support_dispute
);

// Resolve dispute (admin only)
prediction_market.resolve_dispute(
    admin,
    dispute_id,
    true                                     // uphold_dispute
);
```

## Agent Integration

### Reputation System
Agents earn reputation for:
- **Market Creation**: +50 reputation points
- **Betting**: +10 reputation points
- **Correct Predictions**: Bonus reputation

Reputation affects:
- **Market Creation**: Minimum 1000 reputation required
- **Bet Weighting**: Higher reputation = higher influence
- **Rewards**: Reputation-based bonus rewards

### Agent-Only Features
- **create_agent_market**: Create markets with initial liquidity
- **place_bet_reputation_weighted**: Place bets with reputation weighting

## Security Features

### Access Control
- **Admin Functions**: Market resolution, dispute finalization
- **Authentication**: All operations require signature verification
- **Authorization**: Role-based permissions for different operations

### Economic Security
- **Bond Requirements**: Disputes require bonds to prevent spam
- **Slippage Protection**: AMM calculations prevent excessive slippage
- **Liquidity Locks**: Time-based locks on liquidity removal

### Oracle Integration
- **External Oracles**: Integration with trusted oracle providers
- **Data Validation**: Verification of oracle data before resolution
- **Fallback Mechanisms**: Multiple oracle sources for reliability

## Events

The contract emits comprehensive events for monitoring and analytics:

### Market Events
- `market_created`: New market created
- `market_resolved`: Market outcome determined
- `liquidity_added`: Liquidity provided to market
- `liquidity_removed`: Liquidity removed from market

### Betting Events
- `bet_placed`: Bet placed on outcome
- `bet_placed_amm`: AMM-based bet placed
- `reputation_bet_placed`: Reputation-weighted bet placed
- `winnings_claimed`: Winnings claimed by bettor

### Dispute Events
- `dispute_created`: New dispute initiated
- `dispute_vote`: Vote cast on dispute
- `dispute_resolved`: Dispute finalization

### Agent Events
- `agent_market_created`: Agent-created market
- `reputation_updated`: Agent reputation change

## Gas Optimization

### Storage Optimization
- **Packed Storage**: Efficient storage layout for data structures
- **Lazy Loading**: Load data only when needed
- **Storage Reuse**: Reuse storage slots when possible

### Computation Optimization
- **Batch Operations**: Group related operations
- **Caching**: Cache frequently accessed data
- **Efficient Math**: Optimized arithmetic operations

## Testing

The contract includes comprehensive tests covering:
- **Basic Operations**: Market creation, betting, resolution
- **AMM Functionality**: Price discovery, liquidity provision
- **Dispute Resolution**: Full dispute lifecycle
- **Agent Integration**: Reputation-weighted features
- **Edge Cases**: Boundary conditions and error scenarios

## Deployment

### Prerequisites
- Soroban SDK v22.0.0+
- Rust 1.70+
- Stellar network access

### Build Commands
```bash
# Build contract
cargo build --release --target wasm32-unknown-unknown --package prediction-market

# Run tests
cargo test --package prediction-market

# Format code
cargo fmt --package prediction-market

# Lint code
cargo clippy --package prediction-market
```

### Deployment Steps
1. **Initialize Contract**: Deploy to Stellar network
2. **Configure Admin**: Set initial admin address
3. **Initialize Oracle**: Configure oracle bridge
4. **Set Parameters**: Configure market parameters
5. **Verify Deployment**: Test basic functionality

## Integration Examples

### Frontend Integration
```javascript
// Create market using SDK
const marketId = await predictionMarket.createMarket({
  creator: userAddress,
  description: "Market description",
  initialLiquidity: 1000
});

// Place bet
const tokens = await predictionMarket.placeBet({
  bettor: userAddress,
  marketId,
  outcome: 'A',
  amount: 100
});
```

### Analytics Integration
```rust
// Monitor market performance
let market_performance = analytics.get_market_performance(market_id);
let user_accuracy = analytics.get_prediction_accuracy(user_address);
let liquidity_metrics = analytics.get_liquidity_metrics();
```

## Future Enhancements

### Planned Features
- **Multi-Outcome Markets**: Support for more than binary outcomes
- **Dynamic Fees**: Adaptive fee structures
- **Cross-Chain Markets**: Interoperable prediction markets
- **Advanced Oracles**: Machine learning-based predictions

### Scalability Improvements
- **Layer 2 Integration**: Off-chain computation
- **Sharding**: Horizontal scaling for high volume
- **Caching Layer**: Redis-based caching for performance

## License

This contract is part of the StellAIverse project and is licensed under the MIT License.

## Contributing

Contributions are welcome! Please see the [contributing guidelines](../../CONTRIBUTING.md) for details.

## Support

For support and questions:
- GitHub Issues: [StellAIverse/issues](https://github.com/StellAIverse/stellAIverse-contracts/issues)
- Documentation: [StellAIverse Docs](https://docs.stellaiverse.com)
- Community: [Discord](https://discord.gg/stellaiverse)
