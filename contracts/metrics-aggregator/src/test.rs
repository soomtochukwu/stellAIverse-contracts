#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

// ============================================================================
// HELPERS
// ============================================================================

fn setup() -> (Env, Address, MetricsAggregatorClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register(MetricsAggregator, ());
    let client = MetricsAggregatorClient::new(&env, &contract_id);
    client.init_contract(&admin);
    (env, admin, client)
}

// ============================================================================
// INITIALIZATION TESTS
// ============================================================================

#[test]
fn test_init_contract() {
    let (_env, _admin, _client) = setup();
    // init_contract succeeds without panic
}

#[test]
#[should_panic(expected = "Contract already initialized")]
fn test_double_init_panics() {
    let (_env, admin, client) = setup();
    client.init_contract(&admin);
}

// ============================================================================
// RECORD & QUERY TESTS
// ============================================================================

#[test]
fn test_record_and_query_single_metric() {
    let (_env, admin, client) = setup();

    // Record a single metric at timestamp 3600 (hour boundary)
    client.record_metric(&admin, &MetricType::AgentsMinted, &1, &3600);

    // Query hourly bucket
    let result = client.query_metrics(
        &MetricType::AgentsMinted,
        &BucketDuration::Hourly,
        &0,
        &7200,
        &10,
    );

    assert_eq!(result.total_count, 1);
    assert_eq!(result.buckets.len(), 1);

    let bucket = result.buckets.get(0).unwrap();
    assert_eq!(bucket.value, 1);
    assert_eq!(bucket.count, 1);
    assert_eq!(bucket.min, 1);
    assert_eq!(bucket.max, 1);
    assert_eq!(bucket.timestamp, 3600); // aligned to hour
}

#[test]
fn test_record_multiple_aggregates_in_same_bucket() {
    let (_env, admin, client) = setup();

    // Record 3 values in the same hourly bucket (3600–7199)
    client.record_metric(&admin, &MetricType::MarketplaceVolume, &100, &3600);
    client.record_metric(&admin, &MetricType::MarketplaceVolume, &250, &3900);
    client.record_metric(&admin, &MetricType::MarketplaceVolume, &50, &4000);

    let result = client.query_metrics(
        &MetricType::MarketplaceVolume,
        &BucketDuration::Hourly,
        &3600,
        &3600,
        &10,
    );

    assert_eq!(result.total_count, 1);
    let bucket = result.buckets.get(0).unwrap();
    assert_eq!(bucket.value, 400); // 100 + 250 + 50
    assert_eq!(bucket.count, 3);
    assert_eq!(bucket.min, 50);
    assert_eq!(bucket.max, 250);
}

#[test]
fn test_record_across_multiple_hourly_buckets() {
    let (_env, admin, client) = setup();

    // Hour 0 (0–3599)
    client.record_metric(&admin, &MetricType::ExecutionActions, &5, &1000);
    // Hour 1 (3600–7199)
    client.record_metric(&admin, &MetricType::ExecutionActions, &10, &4000);
    // Hour 2 (7200–10799)
    client.record_metric(&admin, &MetricType::ExecutionActions, &15, &8000);

    let result = client.query_metrics(
        &MetricType::ExecutionActions,
        &BucketDuration::Hourly,
        &0,
        &10800,
        &10,
    );

    assert_eq!(result.total_count, 3);
}

#[test]
fn test_daily_bucketing() {
    let (_env, admin, client) = setup();

    // Same day: both fall into daily bucket at timestamp 0
    client.record_metric(&admin, &MetricType::MarketplaceSales, &1, &1000);
    client.record_metric(&admin, &MetricType::MarketplaceSales, &1, &50000);

    let result = client.query_metrics(
        &MetricType::MarketplaceSales,
        &BucketDuration::Daily,
        &0,
        &86400,
        &10,
    );

    assert_eq!(result.total_count, 1);
    let bucket = result.buckets.get(0).unwrap();
    assert_eq!(bucket.value, 2);
    assert_eq!(bucket.count, 2);
}

#[test]
fn test_monthly_bucketing() {
    let (_env, admin, client) = setup();

    // Same month bucket (0 .. 2592000)
    client.record_metric(&admin, &MetricType::GovernanceProposals, &1, &100);
    client.record_metric(&admin, &MetricType::GovernanceProposals, &1, &1_000_000);

    let result = client.query_metrics(
        &MetricType::GovernanceProposals,
        &BucketDuration::Monthly,
        &0,
        &2_592_000,
        &10,
    );

    assert_eq!(result.total_count, 1);
    let bucket = result.buckets.get(0).unwrap();
    assert_eq!(bucket.value, 2);
}

#[test]
fn test_query_empty_range() {
    let (_env, _admin, client) = setup();

    let result = client.query_metrics(
        &MetricType::AgentsMinted,
        &BucketDuration::Hourly,
        &0,
        &7200,
        &10,
    );

    assert_eq!(result.total_count, 0);
    assert_eq!(result.buckets.len(), 0);
    assert!(!result.has_more);
}

#[test]
fn test_query_limit_enforcement() {
    let (_env, admin, client) = setup();

    // Record 5 separate hourly buckets
    for i in 0..5u64 {
        let ts = i * 3600;
        client.record_metric(&admin, &MetricType::EvolutionRequests, &1, &ts);
    }

    // Query with limit 3
    let result = client.query_metrics(
        &MetricType::EvolutionRequests,
        &BucketDuration::Hourly,
        &0,
        &18000,
        &3,
    );

    assert_eq!(result.total_count, 3);
    assert!(result.has_more);
}

// ============================================================================
// USER STATS TESTS
// ============================================================================

#[test]
fn test_record_user_activity_and_get_stats() {
    let (env, admin, client) = setup();

    let user = Address::generate(&env);

    client.record_user_activity(&admin, &user, &UserActivityType::AgentOwned, &2);
    client.record_user_activity(&admin, &user, &UserActivityType::AgentTraded, &1);
    client.record_user_activity(&admin, &user, &UserActivityType::VolumeAdded, &5000);
    client.record_user_activity(&admin, &user, &UserActivityType::AmountSpent, &3000);
    client.record_user_activity(&admin, &user, &UserActivityType::ParticipationScored, &10);

    let stats = client.get_user_stats(&user).unwrap();
    assert_eq!(stats.agents_owned, 2);
    assert_eq!(stats.agents_traded, 1);
    assert_eq!(stats.agents_leased, 0);
    assert_eq!(stats.total_volume, 5000);
    assert_eq!(stats.total_spent, 3000);
    assert_eq!(stats.participation_score, 10);
}

#[test]
fn test_user_stats_increments() {
    let (env, admin, client) = setup();

    let user = Address::generate(&env);

    client.record_user_activity(&admin, &user, &UserActivityType::AgentOwned, &1);
    client.record_user_activity(&admin, &user, &UserActivityType::AgentOwned, &3);

    let stats = client.get_user_stats(&user).unwrap();
    assert_eq!(stats.agents_owned, 4); // 1 + 3
}

#[test]
fn test_get_user_stats_returns_none_for_unknown() {
    let (env, _admin, client) = setup();

    let unknown = Address::generate(&env);
    let stats = client.get_user_stats(&unknown);
    assert!(stats.is_none());
}

// ============================================================================
// TOP-N AGENT TESTS
// ============================================================================

#[test]
fn test_top_agents_sorted_descending() {
    let (_env, admin, client) = setup();

    client.update_agent_score(&admin, &1, &OrderBy::Volume, &500);
    client.update_agent_score(&admin, &2, &OrderBy::Volume, &1000);
    client.update_agent_score(&admin, &3, &OrderBy::Volume, &750);

    let top = client.get_top_agents(&OrderBy::Volume, &10);
    assert_eq!(top.len(), 3);
    assert_eq!(top.get(0).unwrap().agent_id, 2); // 1000
    assert_eq!(top.get(1).unwrap().agent_id, 3); // 750
    assert_eq!(top.get(2).unwrap().agent_id, 1); // 500
}

#[test]
fn test_top_agents_limit() {
    let (_env, admin, client) = setup();

    client.update_agent_score(&admin, &1, &OrderBy::Sales, &100);
    client.update_agent_score(&admin, &2, &OrderBy::Sales, &200);
    client.update_agent_score(&admin, &3, &OrderBy::Sales, &300);

    let top = client.get_top_agents(&OrderBy::Sales, &2);
    assert_eq!(top.len(), 2);
    assert_eq!(top.get(0).unwrap().agent_id, 3);
    assert_eq!(top.get(1).unwrap().agent_id, 2);
}

#[test]
fn test_top_agents_empty() {
    let (_env, _admin, client) = setup();

    let top = client.get_top_agents(&OrderBy::Volume, &10);
    assert_eq!(top.len(), 0);
}

#[test]
fn test_update_agent_score_overwrites() {
    let (_env, admin, client) = setup();

    client.update_agent_score(&admin, &1, &OrderBy::Volume, &500);
    client.update_agent_score(&admin, &1, &OrderBy::Volume, &1500);

    let top = client.get_top_agents(&OrderBy::Volume, &10);
    assert_eq!(top.len(), 1);
    assert_eq!(top.get(0).unwrap().score, 1500);
}

// ============================================================================
// SNAPSHOT TESTS
// ============================================================================

#[test]
fn test_take_and_get_snapshot() {
    let (_env, admin, client) = setup();

    let sid = client.take_snapshot(&admin, &100, &25, &50000, &80, &15, &3);
    assert_eq!(sid, 1);

    let snapshot = client.get_snapshot(&sid).unwrap();
    assert_eq!(snapshot.total_agents, 100);
    assert_eq!(snapshot.active_listings, 25);
    assert_eq!(snapshot.total_volume, 50000);
    assert_eq!(snapshot.total_sales, 80);
    assert_eq!(snapshot.total_evolutions, 15);
    assert_eq!(snapshot.active_proposals, 3);
}

#[test]
fn test_multiple_snapshots() {
    let (_env, admin, client) = setup();

    let s1 = client.take_snapshot(&admin, &10, &5, &1000, &8, &2, &1);
    let s2 = client.take_snapshot(&admin, &20, &10, &5000, &18, &5, &2);

    assert_eq!(s1, 1);
    assert_eq!(s2, 2);

    let snap1 = client.get_snapshot(&s1).unwrap();
    let snap2 = client.get_snapshot(&s2).unwrap();
    assert_eq!(snap1.total_agents, 10);
    assert_eq!(snap2.total_agents, 20);
}

#[test]
fn test_get_nonexistent_snapshot() {
    let (_env, _admin, client) = setup();
    assert!(client.get_snapshot(&999).is_none());
}

// ============================================================================
// PRUNING TESTS
// ============================================================================

#[test]
fn test_prune_old_hourly_buckets() {
    let (_env, admin, client) = setup();

    // Record an old hourly metric (timestamp 0)
    client.record_metric(&admin, &MetricType::AgentsMinted, &1, &0);

    // Record a recent hourly metric
    let recent_ts: u64 = 31_536_000 + 7200; // just beyond 1 year + 2 hours
    client.record_metric(&admin, &MetricType::AgentsMinted, &1, &recent_ts);

    // Prune with before_timestamp far in the future (so old data is aged out)
    let now = recent_ts + 100;
    let pruned = client.prune_metrics(&admin, &now);

    // The hourly bucket at timestamp 0 should be pruned (age > 1 year)
    // Buckets: 3 from first record (hourly/daily/monthly) + 3 from second = 6
    // Hourly at ts=0: age = now - 0 = recent_ts+100 > 31_536_000 → pruned
    // Daily at ts=0: age = now - 0 > 63_072_000? recent_ts+100 ≈ 31_543_300 < 63_072_000 → NOT pruned
    // Monthly at ts=0: never pruned
    assert_eq!(pruned, 1);

    // Old hourly bucket should be gone — query returns nothing at ts=0 for hourly
    let result = client.query_metrics(
        &MetricType::AgentsMinted,
        &BucketDuration::Hourly,
        &0,
        &0,
        &10,
    );
    assert_eq!(result.total_count, 0);

    // Recent hourly bucket should still exist
    let recent_aligned = (recent_ts / 3600) * 3600;
    let result2 = client.query_metrics(
        &MetricType::AgentsMinted,
        &BucketDuration::Hourly,
        &recent_aligned,
        &recent_aligned,
        &10,
    );
    assert_eq!(result2.total_count, 1);
}

#[test]
fn test_prune_monthly_never_pruned() {
    let (_env, admin, client) = setup();

    // Record monthly metric at timestamp 0
    client.record_metric(&admin, &MetricType::MarketplaceSales, &1, &0);

    // Prune with a timestamp far in the future
    let pruned = client.prune_metrics(&admin, &200_000_000);

    // Monthly should survive; only hourly (age > 1yr) and daily (age > 2yr) are candidates
    // At ts=0 with before=200M: hourly pruned, daily pruned (200M > 63M), monthly never
    assert_eq!(pruned, 2); // hourly + daily

    // Monthly bucket at ts=0 still exists
    let result = client.query_metrics(
        &MetricType::MarketplaceSales,
        &BucketDuration::Monthly,
        &0,
        &0,
        &10,
    );
    assert_eq!(result.total_count, 1);
}

// ============================================================================
// PLATFORM SUMMARY TESTS
// ============================================================================

#[test]
fn test_platform_summary() {
    let (_env, admin, client) = setup();

    client.record_metric(&admin, &MetricType::AgentsMinted, &5, &1000);
    client.record_metric(&admin, &MetricType::MarketplaceSales, &3, &2000);
    client.record_metric(&admin, &MetricType::MarketplaceVolume, &10000, &2000);
    client.record_metric(&admin, &MetricType::ExecutionActions, &20, &3000);
    client.record_metric(&admin, &MetricType::EvolutionRequests, &2, &4000);
    client.record_metric(&admin, &MetricType::EvolutionCompleted, &1, &5000);
    client.record_metric(&admin, &MetricType::GovernanceProposals, &4, &6000);

    let summary = client.get_platform_summary();
    assert_eq!(summary.total_agents_minted, 5);
    assert_eq!(summary.total_marketplace_sales, 3);
    assert_eq!(summary.total_marketplace_volume, 10000);
    assert_eq!(summary.total_execution_actions, 20);
    assert_eq!(summary.total_evolution_requests, 2);
    assert_eq!(summary.total_evolution_completed, 1);
    assert_eq!(summary.total_governance_proposals, 4);
}

#[test]
fn test_platform_summary_empty() {
    let (_env, _admin, client) = setup();

    let summary = client.get_platform_summary();
    assert_eq!(summary.total_agents_minted, 0);
    assert_eq!(summary.total_marketplace_sales, 0);
}

// ============================================================================
// AUTH ENFORCEMENT TESTS
// ============================================================================

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_record_metric_non_admin_panics() {
    let (env, _admin, client) = setup();
    let stranger = Address::generate(&env);
    client.record_metric(&stranger, &MetricType::AgentsMinted, &1, &1000);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_record_user_activity_non_admin_panics() {
    let (env, _admin, client) = setup();
    let stranger = Address::generate(&env);
    let user = Address::generate(&env);
    client.record_user_activity(&stranger, &user, &UserActivityType::AgentOwned, &1);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_take_snapshot_non_admin_panics() {
    let (env, _admin, client) = setup();
    let stranger = Address::generate(&env);
    client.take_snapshot(&stranger, &0, &0, &0, &0, &0, &0);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_prune_metrics_non_admin_panics() {
    let (env, _admin, client) = setup();
    let stranger = Address::generate(&env);
    client.prune_metrics(&stranger, &100);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_update_agent_score_non_admin_panics() {
    let (env, _admin, client) = setup();
    let stranger = Address::generate(&env);
    client.update_agent_score(&stranger, &1, &OrderBy::Volume, &100);
}

// ============================================================================
// OVERFLOW SAFETY TESTS
// ============================================================================

#[test]
fn test_large_values_no_overflow() {
    let (_env, admin, client) = setup();

    let large: i128 = i128::MAX / 4;
    client.record_metric(&admin, &MetricType::MarketplaceVolume, &large, &3600);
    client.record_metric(&admin, &MetricType::MarketplaceVolume, &large, &3601);

    let result = client.query_metrics(
        &MetricType::MarketplaceVolume,
        &BucketDuration::Hourly,
        &3600,
        &3600,
        &10,
    );

    let bucket = result.buckets.get(0).unwrap();
    // saturating_add should prevent overflow
    assert!(bucket.value > 0);
    assert_eq!(bucket.count, 2);
}

#[test]
fn test_cumulative_saturation() {
    let (_env, admin, client) = setup();

    let large: i128 = i128::MAX / 2;
    client.record_metric(&admin, &MetricType::AgentsMinted, &large, &0);
    client.record_metric(&admin, &MetricType::AgentsMinted, &large, &3600);
    client.record_metric(&admin, &MetricType::AgentsMinted, &large, &7200);

    let summary = client.get_platform_summary();
    // Should not overflow — saturating_add caps at MAX
    assert!(summary.total_agents_minted > 0);
}

// ============================================================================
// REPUTATION TESTS
// ============================================================================

#[test]
fn test_submit_feedback_updates_reputation() {
    let (env, _admin, client) = setup();

    // Reporter is an external user
    let reporter = Address::generate(&env);
    env.mock_all_auths();

    // Submit feedback for agent 42 with value 100
    let fb_id = client.submit_feedback(
        &reporter,
        &42u64,
        &100i128,
        &super::ReputationReason::Execution,
    );
    assert!(fb_id > 0);

    // Reputation should exist and be at least the submitted value
    let rep = client.get_reputation(&42u64).unwrap();
    assert_eq!(rep.agent_id, 42u64);
    assert!(rep.score >= 100 - 1); // allow small integer rounding
    assert!(rep.count >= 1);
}

#[test]
fn test_dispute_upheld_penalty() {
    let (env, admin, client) = setup();

    // Reporter submits feedback
    let reporter = Address::generate(&env);
    env.mock_all_auths();
    let fb_id = client.submit_feedback(
        &reporter,
        &7u64,
        &200i128,
        &super::ReputationReason::Marketplace,
    );

    // Admin submits dispute resolution (upheld)
    let d_id = client.submit_dispute(&reporter, &fb_id);
    let upheld = client.resolve_dispute(&admin, &d_id, &true);
    assert!(upheld);

    // Reputation should have been penalized (score reduced)
    let rep = client.get_reputation(&7u64).unwrap();
    assert!(rep.score < 200);
}
