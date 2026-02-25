#![allow(unused_imports)]
use soroban_sdk::{contracttype, Address, String, Vec};

// ============================================================================
// METRIC TYPE ENUM
// ============================================================================

/// Categories of metrics tracked across the platform
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum MetricType {
    // Agent metrics
    AgentsMinted = 0,
    AgentsActive = 1,
    AgentsInactive = 2,

    // Marketplace metrics
    MarketplaceListings = 10,
    MarketplaceSales = 11,
    MarketplaceVolume = 12,
    MarketplaceAvgPrice = 13,

    // Execution Hub metrics
    ExecutionActions = 20,
    ExecutionRateLimitHits = 21,
    ExecutionAnomalies = 22,

    // Evolution metrics
    EvolutionRequests = 30,
    EvolutionCompleted = 31,
    EvolutionAvgTime = 32,

    // Governance metrics
    GovernanceProposals = 40,
    GovernanceExecuted = 41,
    GovernanceParticipation = 42,
}

// ============================================================================
// BUCKET DURATION ENUM
// ============================================================================

/// Time granularity for aggregation buckets
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum BucketDuration {
    Hourly = 0,
    Daily = 1,
    Monthly = 2,
}

// ============================================================================
// ORDER BY ENUM
// ============================================================================

/// Ranking criteria for top-N agent queries
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum OrderBy {
    Volume = 0,
    Sales = 1,
    EvolutionLevel = 2,
}

// ============================================================================
// USER ACTIVITY TYPE ENUM
// ============================================================================

/// Types of user activities that can be recorded
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum UserActivityType {
    AgentOwned = 0,
    AgentTraded = 1,
    AgentLeased = 2,
    VolumeAdded = 3,
    AmountSpent = 4,
    ParticipationScored = 5,
}

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// Time-series aggregation bucket for a specific metric
#[contracttype]
#[derive(Clone, Debug)]
pub struct MetricsBucket {
    pub bucket_id: u64,
    pub timestamp: u64,
    pub duration: BucketDuration,
    pub metric_type: MetricType,
    pub value: i128,
    pub count: u32,
    pub min: i128,
    pub max: i128,
}

/// Per-user analytics summary
#[contracttype]
#[derive(Clone, Debug)]
pub struct UserStats {
    pub user: Address,
    pub agents_owned: u32,
    pub agents_traded: u32,
    pub agents_leased: u32,
    pub total_volume: i128,
    pub total_spent: i128,
    pub participation_score: u32,
    pub last_active: u64,
}

/// Point-in-time platform snapshot for historical analysis
#[contracttype]
#[derive(Clone, Debug)]
pub struct MetricSnapshot {
    pub snapshot_id: u64,
    pub timestamp: u64,
    pub total_agents: u64,
    pub active_listings: u64,
    pub total_volume: i128,
    pub total_sales: u64,
    pub total_evolutions: u64,
    pub active_proposals: u32,
}

/// Paginated query result for metrics
#[contracttype]
#[derive(Clone, Debug)]
pub struct MetricsQueryResult {
    pub buckets: Vec<MetricsBucket>,
    pub total_count: u32,
    pub has_more: bool,
}

/// Agent ranking entry for top-N queries
#[contracttype]
#[derive(Clone, Debug)]
pub struct AgentRanking {
    pub agent_id: u64,
    pub score: i128,
}

/// Platform-wide summary computed from latest counters
#[contracttype]
#[derive(Clone, Debug)]
pub struct PlatformSummary {
    pub timestamp: u64,
    pub total_agents_minted: i128,
    pub total_marketplace_sales: i128,
    pub total_marketplace_volume: i128,
    pub total_execution_actions: i128,
    pub total_evolution_requests: i128,
    pub total_evolution_completed: i128,
    pub total_governance_proposals: i128,
}
