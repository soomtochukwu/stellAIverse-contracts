#![no_std]

mod storage;
pub mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};
use stellai_lib::{admin, ADMIN_KEY};

use storage::*;
use types::*;

#[contract]
pub struct MetricsAggregator;

#[contractimpl]
impl MetricsAggregator {
    // ========================================================================
    // INITIALIZATION
    // ========================================================================

    /// Initialize the metrics aggregator (one-time setup)
    pub fn init_contract(env: Env, admin_addr: Address) {
        let existing = env
            .storage()
            .instance()
            .get::<_, Address>(&Symbol::new(&env, ADMIN_KEY));
        if existing.is_some() {
            panic!("Contract already initialized");
        }

        admin_addr.require_auth();
        env.storage()
            .instance()
            .set(&Symbol::new(&env, ADMIN_KEY), &admin_addr);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, BUCKET_COUNTER_KEY), &0u64);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, SNAPSHOT_COUNTER_KEY), &0u64);

        env.events().publish(
            (Symbol::new(&env, "metrics_init"),),
            (admin_addr,),
        );
    }

    // ========================================================================
    // METRIC RECORDING
    // ========================================================================

    /// Record a metric data point. Automatically aggregates into hourly, daily,
    /// and monthly buckets. Admin-only.
    ///
    /// # Arguments
    /// * `caller` – Must be admin
    /// * `metric_type` – Which metric to record
    /// * `value` – The metric value (e.g., 1 for a count, or a price amount)
    /// * `timestamp` – Ledger timestamp of the event
    pub fn record_metric(
        env: Env,
        caller: Address,
        metric_type: MetricType,
        value: i128,
        timestamp: u64,
    ) {
        caller.require_auth();
        Self::verify_admin(&env, &caller);

        // Update cumulative counter
        add_cumulative(&env, metric_type, value);

        // Insert/update bucket for each granularity
        Self::upsert_bucket(&env, metric_type, BucketDuration::Hourly, value, timestamp);
        Self::upsert_bucket(&env, metric_type, BucketDuration::Daily, value, timestamp);
        Self::upsert_bucket(&env, metric_type, BucketDuration::Monthly, value, timestamp);

        env.events().publish(
            (Symbol::new(&env, "metric_recorded"),),
            (metric_type as u32, value, timestamp),
        );
    }

    /// Record user activity. Increments the appropriate field in UserStats.
    /// Admin-only.
    pub fn record_user_activity(
        env: Env,
        caller: Address,
        user: Address,
        activity_type: UserActivityType,
        value: i128,
    ) {
        caller.require_auth();
        Self::verify_admin(&env, &caller);

        let now = env.ledger().timestamp();
        let mut stats = storage::get_user_stats(&env, &user).unwrap_or(UserStats {
            user: user.clone(),
            agents_owned: 0,
            agents_traded: 0,
            agents_leased: 0,
            total_volume: 0,
            total_spent: 0,
            participation_score: 0,
            last_active: 0,
        });

        stats.last_active = now;

        match activity_type {
            UserActivityType::AgentOwned => {
                stats.agents_owned = stats.agents_owned.saturating_add(value as u32);
            }
            UserActivityType::AgentTraded => {
                stats.agents_traded = stats.agents_traded.saturating_add(value as u32);
            }
            UserActivityType::AgentLeased => {
                stats.agents_leased = stats.agents_leased.saturating_add(value as u32);
            }
            UserActivityType::VolumeAdded => {
                stats.total_volume = stats.total_volume.saturating_add(value);
            }
            UserActivityType::AmountSpent => {
                stats.total_spent = stats.total_spent.saturating_add(value);
            }
            UserActivityType::ParticipationScored => {
                stats.participation_score =
                    stats.participation_score.saturating_add(value as u32);
            }
        }

        storage::store_user_stats(&env, &stats);

        env.events().publish(
            (Symbol::new(&env, "user_activity"),),
            (user, activity_type as u32, value),
        );
    }

    /// Update an agent's score for top-N ranking. Admin-only.
    pub fn update_agent_score(
        env: Env,
        caller: Address,
        agent_id: u64,
        order_by: OrderBy,
        score: i128,
    ) {
        caller.require_auth();
        Self::verify_admin(&env, &caller);

        storage::store_agent_score(&env, agent_id, order_by, score);
        storage::add_agent_to_scoreboard(&env, order_by, agent_id);

        env.events().publish(
            (Symbol::new(&env, "agent_score"),),
            (agent_id, order_by as u32, score),
        );
    }

    // ========================================================================
    // QUERIES
    // ========================================================================

    /// Query aggregated metrics for a given type and duration within a time range.
    ///
    /// Iterates bucket indices for each aligned timestamp in [start_time, end_time].
    pub fn query_metrics(
        env: Env,
        metric_type: MetricType,
        bucket_duration: BucketDuration,
        start_time: u64,
        end_time: u64,
        limit: u32,
    ) -> MetricsQueryResult {
        let effective_limit = if limit == 0 || limit > MAX_QUERY_LIMIT {
            MAX_QUERY_LIMIT
        } else {
            limit
        };

        let period = duration_seconds(bucket_duration);
        let aligned_start = align_timestamp(start_time, bucket_duration);
        let aligned_end = align_timestamp(end_time, bucket_duration);

        let mut buckets: Vec<MetricsBucket> = Vec::new(&env);
        let mut count: u32 = 0;
        let mut ts = aligned_start;

        while ts <= aligned_end && count < effective_limit {
            if let Some(bid) = get_bucket_index(&env, metric_type, bucket_duration, ts) {
                if let Some(bucket) = get_bucket(&env, bid) {
                    buckets.push_back(bucket);
                    count += 1;
                }
            }
            // Advance to next period; guard against zero period
            ts = ts.saturating_add(period);
            if period == 0 {
                break;
            }
        }

        let has_more = ts <= aligned_end && count == effective_limit;

        MetricsQueryResult {
            buckets,
            total_count: count,
            has_more,
        }
    }

    /// Get analytics summary for a specific user.
    pub fn get_user_stats(env: Env, user: Address) -> Option<UserStats> {
        storage::get_user_stats(&env, &user)
    }

    /// Get top-N agents ranked by the specified criterion.
    pub fn get_top_agents(env: Env, order_by: OrderBy, limit: u32) -> Vec<AgentRanking> {
        let effective_limit = if limit == 0 || limit > MAX_QUERY_LIMIT {
            MAX_QUERY_LIMIT
        } else {
            limit
        };

        let agent_ids = storage::get_agent_scoreboard(&env, order_by);
        let mut rankings: Vec<AgentRanking> = Vec::new(&env);

        // Collect all scores
        for i in 0..agent_ids.len() {
            if let Some(aid) = agent_ids.get(i) {
                let score = storage::get_agent_score(&env, aid, order_by).unwrap_or(0);
                rankings.push_back(AgentRanking {
                    agent_id: aid,
                    score,
                });
            }
        }

        // Simple insertion sort descending (suitable for on-chain with bounded data)
        let len = rankings.len();
        if len > 1 {
            let mut i = 1u32;
            while i < len {
                let current = rankings.get(i).unwrap();
                let mut j = i;
                while j > 0 {
                    let prev = rankings.get(j - 1).unwrap();
                    if prev.score < current.score {
                        rankings.set(j, prev);
                        j -= 1;
                    } else {
                        break;
                    }
                }
                rankings.set(j, current);
                i += 1;
            }
        }

        // Truncate to limit
        let mut result: Vec<AgentRanking> = Vec::new(&env);
        let take = if len < effective_limit { len } else { effective_limit };
        for i in 0..take {
            if let Some(r) = rankings.get(i) {
                result.push_back(r);
            }
        }

        result
    }

    // ========================================================================
    // SNAPSHOTS
    // ========================================================================

    /// Take a point-in-time snapshot of platform-wide metrics. Admin-only.
    pub fn take_snapshot(
        env: Env,
        caller: Address,
        total_agents: u64,
        active_listings: u64,
        total_volume: i128,
        total_sales: u64,
        total_evolutions: u64,
        active_proposals: u32,
    ) -> u64 {
        caller.require_auth();
        Self::verify_admin(&env, &caller);

        let snapshot_id = increment_counter(&env, SNAPSHOT_COUNTER_KEY);
        let snapshot = MetricSnapshot {
            snapshot_id,
            timestamp: env.ledger().timestamp(),
            total_agents,
            active_listings,
            total_volume,
            total_sales,
            total_evolutions,
            active_proposals,
        };

        storage::store_snapshot(&env, &snapshot);

        env.events().publish(
            (Symbol::new(&env, "snapshot_taken"),),
            (snapshot_id,),
        );

        snapshot_id
    }

    /// Retrieve a specific snapshot by ID.
    pub fn get_snapshot(env: Env, snapshot_id: u64) -> Option<MetricSnapshot> {
        storage::get_snapshot(&env, snapshot_id)
    }

    // ========================================================================
    // PRUNING
    // ========================================================================

    /// Prune old metric buckets to reclaim storage. Admin-only.
    ///
    /// Deletes hourly buckets older than `RETENTION_HOURLY_SECONDS` and
    /// daily buckets older than `RETENTION_DAILY_SECONDS` relative to
    /// `before_timestamp`. Monthly buckets are never pruned.
    ///
    /// Returns the number of buckets removed.
    pub fn prune_metrics(env: Env, caller: Address, before_timestamp: u64) -> u32 {
        caller.require_auth();
        Self::verify_admin(&env, &caller);

        let total_buckets = get_counter(&env, BUCKET_COUNTER_KEY);
        let mut pruned = 0u32;

        for bid in 1..=total_buckets {
            if let Some(bucket) = get_bucket(&env, bid) {
                let should_prune = match bucket.duration {
                    BucketDuration::Hourly => {
                        before_timestamp
                            .saturating_sub(bucket.timestamp)
                            > RETENTION_HOURLY_SECONDS
                    }
                    BucketDuration::Daily => {
                        before_timestamp
                            .saturating_sub(bucket.timestamp)
                            > RETENTION_DAILY_SECONDS
                    }
                    BucketDuration::Monthly => false, // Never prune monthly
                };

                if should_prune {
                    remove_bucket_index(
                        &env,
                        bucket.metric_type,
                        bucket.duration,
                        bucket.timestamp,
                    );
                    remove_bucket(&env, bid);
                    pruned += 1;
                }
            }
        }

        env.events().publish(
            (Symbol::new(&env, "metrics_pruned"),),
            (pruned, before_timestamp),
        );

        pruned
    }

    // ========================================================================
    // PLATFORM SUMMARY
    // ========================================================================

    /// Get a platform-wide summary from cumulative counters.
    pub fn get_platform_summary(env: Env) -> PlatformSummary {
        PlatformSummary {
            timestamp: env.ledger().timestamp(),
            total_agents_minted: get_cumulative(&env, MetricType::AgentsMinted),
            total_marketplace_sales: get_cumulative(&env, MetricType::MarketplaceSales),
            total_marketplace_volume: get_cumulative(&env, MetricType::MarketplaceVolume),
            total_execution_actions: get_cumulative(&env, MetricType::ExecutionActions),
            total_evolution_requests: get_cumulative(&env, MetricType::EvolutionRequests),
            total_evolution_completed: get_cumulative(&env, MetricType::EvolutionCompleted),
            total_governance_proposals: get_cumulative(&env, MetricType::GovernanceProposals),
        }
    }

    // ========================================================================
    // INTERNAL HELPERS
    // ========================================================================

    fn verify_admin(env: &Env, caller: &Address) {
        if admin::verify_admin(env, caller).is_err() {
            panic!("Unauthorized: caller is not admin");
        }
    }

    /// Insert or update a bucket for the given metric, duration, and timestamp.
    fn upsert_bucket(
        env: &Env,
        metric_type: MetricType,
        duration: BucketDuration,
        value: i128,
        timestamp: u64,
    ) {
        let aligned_ts = align_timestamp(timestamp, duration);

        if let Some(existing_id) = get_bucket_index(env, metric_type, duration, aligned_ts) {
            // Update existing bucket
            if let Some(mut bucket) = get_bucket(env, existing_id) {
                bucket.value = bucket.value.saturating_add(value);
                bucket.count = bucket.count.saturating_add(1);
                if value < bucket.min {
                    bucket.min = value;
                }
                if value > bucket.max {
                    bucket.max = value;
                }
                store_bucket(env, &bucket);
            }
        } else {
            // Create new bucket
            let bucket_id = increment_counter(env, BUCKET_COUNTER_KEY);
            let bucket = MetricsBucket {
                bucket_id,
                timestamp: aligned_ts,
                duration,
                metric_type,
                value,
                count: 1,
                min: value,
                max: value,
            };
            store_bucket(env, &bucket);
            set_bucket_index(env, metric_type, duration, aligned_ts, bucket_id);
        }
    }
}
