#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Bytes, Env, IntoVal, String,
    Symbol, Vec,
};
#[cfg(test)]
mod prop_tests;
use stellai_lib::{
    admin, errors::ContractError, storage_keys::EXEC_CTR_KEY, validation, ADMIN_KEY,
    DEFAULT_RATE_LIMIT_OPERATIONS, DEFAULT_RATE_LIMIT_WINDOW_SECONDS, MAX_DATA_SIZE,
    MAX_HISTORY_QUERY_LIMIT, MAX_HISTORY_SIZE, MAX_STRING_LENGTH,
};

#[derive(Clone)]
#[contracttype]
pub struct RuleKey {
    pub agent_id: u64,
    pub rule_name: String,
}

#[derive(Clone)]
#[contracttype]
pub struct OperatorData {
    pub operator: Address,
    pub expires_at: u64,
}

const AGENT_NFT_KEY: &str = "agent_nft";

// Rate limit configuration storage keys
const GLOBAL_RATE_LIMIT_KEY: Symbol = symbol_short!("rate_gl");
const AGENT_RATE_LIMIT_PREFIX: Symbol = symbol_short!("rate_ag");
const BYPASS_PREFIX: Symbol = symbol_short!("bypass");

#[derive(Clone)]
#[contracttype]
pub struct ActionRecord {
    pub execution_id: u64,
    pub agent_id: u64,
    pub action: String,
    pub executor: Address,
    pub timestamp: u64,
    pub nonce: u64,
    /// Cryptographic hash of execution data for off-chain verification (Issue #10)
    pub execution_hash: Bytes,
}

/// Immutable execution receipt for off-chain proof storage (Issue #10)
/// Receipts are stored separately and cannot be modified after creation
#[derive(Clone)]
#[contracttype]
pub struct ExecutionReceipt {
    pub execution_id: u64,
    pub agent_id: u64,
    pub action: String,
    pub executor: Address,
    pub timestamp: u64,
    pub execution_hash: Bytes,
    pub created_at: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct RateLimitData {
    pub last_reset: u64,
    pub count: u32,
}

/// Configurable rate limit: max operations per window_seconds (per-agent or global).
#[derive(Clone)]
#[contracttype]
pub struct RateLimitConfig {
    pub operations: u32,
    pub window_seconds: u64,
}

/// Audit record for emergency rate limit bypass (admin only).
#[derive(Clone)]
#[contracttype]
pub struct BypassRecord {
    pub valid_until: u64,
    pub reason: String,
}

#[contract]
pub struct ExecutionHub;

#[contractimpl]
impl ExecutionHub {
    // Initialize contract with admin and AgentNFT address
    pub fn initialize(env: Env, admin: Address, agent_nft: Address) {
        if env.storage().instance().has(&ADMIN_KEY) {
            panic!("Contract already initialized");
        }

        admin.require_auth();
        env.storage().instance().set(&ADMIN_KEY, &admin);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, AGENT_NFT_KEY), &agent_nft);
        env.storage().instance().set(&EXEC_CTR_KEY, &0u64);

        let global_rate_limit = RateLimitConfig {
            operations: DEFAULT_RATE_LIMIT_OPERATIONS,
            window_seconds: DEFAULT_RATE_LIMIT_WINDOW_SECONDS,
        };
        env.storage()
            .instance()
            .set(&GLOBAL_RATE_LIMIT_KEY, &global_rate_limit);

        env.events()
            .publish((symbol_short!("init"),), (admin, agent_nft));
    }

    // Get current execution counter
    pub fn get_execution_counter(env: Env) -> u64 {
        env.storage().instance().get(&EXEC_CTR_KEY).unwrap_or(0u64)
    }

    // Increment execution ID
    fn next_execution_id(env: &Env) -> u64 {
        let current: u64 = env.storage().instance().get(&EXEC_CTR_KEY).unwrap_or(0u64);
        let next = current.saturating_add(1);
        if next == 0 {
            panic!("Execution ID overflow");
        }
        env.storage().instance().set(&EXEC_CTR_KEY, &next);
        next
    }

    // Register execution rule for agent
    pub fn register_rule(
        env: Env,
        agent_id: u64,
        owner: Address,
        rule_name: String,
        rule_data: Bytes,
    ) {
        owner.require_auth();

        Self::validate_agent_id(agent_id);
        Self::validate_string_length(&rule_name, "Rule name");
        Self::validate_data_size(&rule_data, "Rule data");

        let rule_key = RuleKey {
            agent_id,
            rule_name: rule_name.clone(),
        };
        let timestamp = env.ledger().timestamp();

        env.storage().instance().set(&rule_key, &rule_data);
        env.events().publish(
            (symbol_short!("rule_reg"),),
            (agent_id, rule_name, owner, timestamp),
        );
    }

    // Revoke existing rule
    pub fn revoke_rule(env: Env, agent_id: u64, owner: Address, rule_name: String) {
        owner.require_auth();
        Self::validate_agent_id(agent_id);

        let rule_key = RuleKey {
            agent_id,
            rule_name: rule_name.clone(),
        };
        env.storage().instance().remove(&rule_key);

        env.events()
            .publish((symbol_short!("rule_rev"),), (agent_id, rule_name, owner));
    }

    // Authorize an operator (lessee) for an agent
    pub fn authorize_operator(
        env: Env,
        agent_id: u64,
        owner: Address,
        operator: Address,
        duration_seconds: u64,
    ) {
        owner.require_auth();
        Self::validate_agent_id(agent_id);

        // Verify owner via AgentNFT
        let actual_owner = Self::get_agent_owner(&env, agent_id);
        if owner != actual_owner {
            panic!("Unauthorized: caller is not agent owner");
        }

        let expires_at = env.ledger().timestamp() + duration_seconds;
        let operator_data = OperatorData {
            operator: operator.clone(),
            expires_at,
        };

        let op_key = symbol_short!("op");
        let agent_op_key = (op_key, agent_id);
        env.storage().instance().set(&agent_op_key, &operator_data);

        env.events().publish(
            (symbol_short!("auth_op"),),
            (agent_id, owner, operator, expires_at),
        );
    }

    // Revoke an operator
    pub fn revoke_operator(env: Env, agent_id: u64, owner: Address) {
        owner.require_auth();
        Self::validate_agent_id(agent_id);

        // Verify owner via AgentNFT
        let actual_owner = Self::get_agent_owner(&env, agent_id);
        if owner != actual_owner {
            panic!("Unauthorized: caller is not agent owner");
        }

        let op_key = symbol_short!("op");
        let agent_op_key = (op_key, agent_id);
        env.storage().instance().remove(&agent_op_key);

        env.events()
            .publish((symbol_short!("rev_op"),), (agent_id, owner));
    }

    // Get rule data
    pub fn get_rule(env: Env, agent_id: u64, rule_name: String) -> Option<Bytes> {
        Self::validate_agent_id(agent_id);
        let rule_key = RuleKey {
            agent_id,
            rule_name,
        };
        env.storage().instance().get(&rule_key)
    }

    /// Execute action with validation, replay protection, and proof storage (Issue #10)
    ///
    /// # Arguments
    /// * `agent_id` - The agent executing the action
    /// * `executor` - Address of the executor
    /// * `action` - Action name/type
    /// * `parameters` - Action parameters
    /// * `nonce` - Replay protection nonce
    /// * `execution_hash` - Cryptographic hash for off-chain verification
    ///
    /// # Returns
    /// The execution ID for this action
    pub fn execute_action(
        env: Env,
        agent_id: u64,
        executor: Address,
        action: String,
        parameters: Bytes,
        nonce: u64,
        execution_hash: Bytes,
    ) -> u64 {
        executor.require_auth();

        Self::validate_agent_id(agent_id);

        // Permission Check: Owner or Authorized Operator
        // 1. Check if executor is owner
        let owner = Self::get_agent_owner(&env, agent_id);
        let is_owner = executor == owner;

        // 2. If not owner, check if authorized operator
        if !is_owner {
            let op_key = symbol_short!("op");
            let agent_op_key = (op_key, agent_id);
            if let Some(op_data) = env
                .storage()
                .instance()
                .get::<_, OperatorData>(&agent_op_key)
            {
                if op_data.operator != executor {
                    panic!("Unauthorized: executor is not owner or operator");
                }
                if env.ledger().timestamp() > op_data.expires_at {
                    panic!("Unauthorized: operator authorization expired");
                }
            } else {
                panic!("Unauthorized: executor is not owner or operator");
            }
        }
        Self::validate_string_length(&action, "Action name");
        Self::validate_data_size(&parameters, "Parameters");
        Self::validate_data_size(&execution_hash, "Execution hash");

        // Replay protection
        let stored_nonce = Self::get_action_nonce(&env, agent_id);
        if nonce <= stored_nonce {
            panic!("Invalid nonce: replay protection triggered");
        }

        // Rate limiting (uses configurable global/per-agent config; bypass if admin set one)
        Self::check_rate_limit(&env, agent_id);

        let execution_id = Self::next_execution_id(&env);
        let timestamp = env.ledger().timestamp();

        Self::set_action_nonce(&env, agent_id, nonce);
        Self::record_action_in_history(
            &env,
            agent_id,
            execution_id,
            &action,
            &executor,
            nonce,
            &execution_hash,
        );
        Self::store_execution_receipt(
            &env,
            execution_id,
            agent_id,
            &action,
            &executor,
            timestamp,
            &execution_hash,
        );

        env.events().publish(
            (symbol_short!("act_exec"),),
            (
                execution_id,
                agent_id,
                action.clone(),
                executor.clone(),
                timestamp,
                nonce,
                execution_hash.clone(),
            ),
        );

        execution_id
    }

    // Get execution history
    pub fn get_history(env: Env, agent_id: u64, limit: u32) -> Vec<ActionRecord> {
        Self::validate_agent_id(agent_id);

        if limit > MAX_HISTORY_QUERY_LIMIT {
            panic!("Limit exceeds maximum allowed (500)");
        }

        let history_key = symbol_short!("hist");
        let agent_key = (history_key, agent_id);
        let history: Vec<ActionRecord> = env
            .storage()
            .instance()
            .get(&agent_key)
            .unwrap_or_else(|| Vec::new(&env));

        let mut result = Vec::new(&env);
        let start_idx = if history.len() > limit {
            history.len() - limit
        } else {
            0
        };

        for i in start_idx..history.len() {
            if let Some(item) = history.get(i) {
                result.push_back(item);
            }
        }

        result
    }

    // Get total action count
    pub fn get_action_count(env: Env, agent_id: u64) -> u32 {
        Self::validate_agent_id(agent_id);
        let history_key = symbol_short!("hist");
        let agent_key = (history_key, agent_id);
        let history: Vec<ActionRecord> = env
            .storage()
            .instance()
            .get(&agent_key)
            .unwrap_or_else(|| Vec::new(&env));
        history.len()
    }

    /// Get execution receipt by execution ID (Issue #10)
    /// Read-only getter for immutable execution proofs
    /// Returns None if the execution ID doesn't exist
    pub fn get_execution_receipt(env: Env, execution_id: u64) -> Option<ExecutionReceipt> {
        let receipt_key = symbol_short!("receipt");
        let exec_receipt_key = (receipt_key, execution_id);
        env.storage().instance().get(&exec_receipt_key)
    }

    /// Get agent ID for a given execution ID (Issue #10)
    /// Provides reverse lookup from execution to agent
    /// Returns None if the execution ID doesn't exist
    pub fn get_agent_for_execution(env: Env, execution_id: u64) -> Option<u64> {
        let exec_agent_key = symbol_short!("exagent");
        let exec_to_agent_key = (exec_agent_key, execution_id);
        env.storage().instance().get(&exec_to_agent_key)
    }

    /// Get all execution receipts for an agent (Issue #10)
    /// Returns a list of execution receipts for the given agent
    pub fn get_agent_receipts(env: Env, agent_id: u64, limit: u32) -> Vec<ExecutionReceipt> {
        Self::validate_agent_id(agent_id);

        if limit > MAX_HISTORY_QUERY_LIMIT {
            panic!("Limit exceeds maximum allowed (500)");
        }

        // Get action history and extract receipts
        let history_key = symbol_short!("hist");
        let agent_key = (history_key, agent_id);
        let history: Vec<ActionRecord> = env
            .storage()
            .instance()
            .get(&agent_key)
            .unwrap_or_else(|| Vec::new(&env));

        let mut receipts = Vec::new(&env);
        let start_idx = if history.len() > limit {
            history.len() - limit
        } else {
            0
        };

        for i in start_idx..history.len() {
            if let Some(record) = history.get(i) {
                if let Some(receipt) = Self::get_execution_receipt(env.clone(), record.execution_id)
                {
                    receipts.push_back(receipt);
                }
            }
        }

        receipts
    }

    // Get admin address
    pub fn get_admin(env: Env) -> Address {
        admin::get_admin(&env).unwrap_or_else(|_| panic!("Admin not set"))
    }

    /// Returns the effective rate limit config for an agent (per-agent override or global).
    pub fn get_rate_limit(env: Env, agent_id: u64) -> RateLimitConfig {
        Self::get_effective_rate_limit(&env, agent_id)
    }

    /// Admin: set global rate limit (applies to all agents without an override).
    pub fn set_global_rate_limit(env: Env, admin: Address, ops: u32, window_secs: u64) {
        admin.require_auth();
        Self::verify_admin(&env, &admin);
        Self::validate_rate_limit_config(ops, window_secs);

        let config = RateLimitConfig {
            operations: ops,
            window_seconds: window_secs,
        };
        env.storage()
            .instance()
            .set(&GLOBAL_RATE_LIMIT_KEY, &config);
        // agent_id 0 denotes global in events
        env.events()
            .publish((symbol_short!("rate_cfg"),), (0u64, ops, window_secs));
    }

    /// Admin: set per-agent rate limit override (e.g. for trusted oracles or high-frequency agents).
    pub fn set_agent_rate_limit(
        env: Env,
        admin: Address,
        agent_id: u64,
        ops: u32,
        window_secs: u64,
    ) {
        admin.require_auth();
        Self::verify_admin(&env, &admin);
        Self::validate_agent_id(agent_id);
        Self::validate_rate_limit_config(ops, window_secs);

        let config = RateLimitConfig {
            operations: ops,
            window_seconds: window_secs,
        };
        let agent_key = (AGENT_RATE_LIMIT_PREFIX, agent_id);
        env.storage().instance().set(&agent_key, &config);
        env.events()
            .publish((symbol_short!("rate_cfg"),), (agent_id, ops, window_secs));
    }

    /// Admin: remove per-agent override; agent falls back to global config.
    pub fn reset_agent_rate_limit(env: Env, admin: Address, agent_id: u64) {
        admin.require_auth();
        Self::verify_admin(&env, &admin);
        Self::validate_agent_id(agent_id);

        let agent_key = (AGENT_RATE_LIMIT_PREFIX, agent_id);
        env.storage().instance().remove(&agent_key);
        env.events()
            .publish((symbol_short!("rate_rst"),), (agent_id,));
    }

    /// Admin: emergency rate limit bypass for a specific agent (with audit log).
    pub fn set_rate_limit_bypass(
        env: Env,
        admin: Address,
        agent_id: u64,
        reason: String,
        valid_until: u64,
    ) {
        admin.require_auth();
        Self::verify_admin(&env, &admin);
        Self::validate_agent_id(agent_id);
        let now = env.ledger().timestamp();
        if valid_until <= now {
            panic!("valid_until must be in the future");
        }

        let record = BypassRecord {
            valid_until,
            reason: reason.clone(),
        };
        let bypass_key = (BYPASS_PREFIX, agent_id);
        env.storage().instance().set(&bypass_key, &record);
        env.events()
            .publish((symbol_short!("bypass_on"),), (agent_id, reason));
    }

    /// Admin: clear emergency bypass for an agent.
    pub fn clear_rate_limit_bypass(env: Env, admin: Address, agent_id: u64) {
        admin.require_auth();
        Self::verify_admin(&env, &admin);
        Self::validate_agent_id(agent_id);

        let bypass_key = (BYPASS_PREFIX, agent_id);
        env.storage().instance().remove(&bypass_key);
        env.events()
            .publish((symbol_short!("byp_off"),), (agent_id,));
    }

    // Transfer admin rights
    pub fn transfer_admin(env: Env, current_admin: Address, new_admin: Address) {
        admin::transfer_admin(&env, &current_admin, &new_admin)
            .unwrap_or_else(|_| panic!("Unauthorized: caller is not admin"));
        env.events()
            .publish((symbol_short!("adm_xfer"),), (current_admin, new_admin));
    }

    // Helper: verify admin
    fn verify_admin(env: &Env, caller: &Address) {
        if admin::verify_admin(env, caller) == Err(ContractError::Unauthorized) {
            panic!("Unauthorized: caller is not admin");
        }
    }

    // Helper: validate rate limit config (ops and window must be positive)
    fn validate_rate_limit_config(ops: u32, window_secs: u64) {
        if ops == 0 {
            panic!("operations must be greater than 0");
        }
        if window_secs == 0 {
            panic!("window_seconds must be greater than 0");
        }
    }

    // Helper: get effective rate limit for agent (override or global)
    fn get_effective_rate_limit(env: &Env, agent_id: u64) -> RateLimitConfig {
        let agent_key = (AGENT_RATE_LIMIT_PREFIX, agent_id);
        if let Some(config) = env
            .storage()
            .instance()
            .get::<_, RateLimitConfig>(&agent_key)
        {
            return config;
        }
        env.storage()
            .instance()
            .get(&GLOBAL_RATE_LIMIT_KEY)
            .unwrap_or_else(|| panic!("Global rate limit not set"))
    }

    // Helper: check if agent has an active bypass
    fn has_active_bypass(env: &Env, agent_id: u64) -> bool {
        let bypass_key = (BYPASS_PREFIX, agent_id);
        let now = env.ledger().timestamp();
        if let Some(record) = env.storage().instance().get::<_, BypassRecord>(&bypass_key) {
            return now < record.valid_until;
        }
        false
    }

    // Helper: validate agent ID
    fn validate_agent_id(agent_id: u64) {
        if validation::validate_nonzero_id(agent_id).is_err() {
            panic!("Invalid agent ID: must be non-zero");
        }
    }

    // Helper: validate string length
    fn validate_string_length(s: &String, _field_name: &str) {
        if s.len() > MAX_STRING_LENGTH {
            panic!("String exceeds maximum length");
        }
    }

    // Helper: validate data size
    fn validate_data_size(data: &Bytes, _field_name: &str) {
        if data.len() > MAX_DATA_SIZE {
            panic!("Data exceeds maximum size");
        }
    }

    // Helper: get nonce
    fn get_action_nonce(env: &Env, agent_id: u64) -> u64 {
        let nonce_key = symbol_short!("nonce");
        let agent_nonce_key = (nonce_key, agent_id);
        env.storage().instance().get(&agent_nonce_key).unwrap_or(0)
    }

    // Helper: set nonce
    fn set_action_nonce(env: &Env, agent_id: u64, nonce: u64) {
        let nonce_key = symbol_short!("nonce");
        let agent_nonce_key = (nonce_key, agent_id);
        env.storage().instance().set(&agent_nonce_key, &nonce);
    }

    // Helper: record action in history with execution hash (Issue #10)
    fn record_action_in_history(
        env: &Env,
        agent_id: u64,
        execution_id: u64,
        action: &String,
        executor: &Address,
        nonce: u64,
        execution_hash: &Bytes,
    ) {
        let history_key = symbol_short!("hist");
        let agent_key = (history_key, agent_id);

        let mut history: Vec<ActionRecord> = env
            .storage()
            .instance()
            .get(&agent_key)
            .unwrap_or_else(|| Vec::new(env));

        if history.len() >= MAX_HISTORY_SIZE {
            panic!("Action history limit exceeded");
        }

        let timestamp = env.ledger().timestamp();
        let record = ActionRecord {
            execution_id,
            agent_id,
            action: action.clone(),
            executor: executor.clone(),
            timestamp,
            nonce,
            execution_hash: execution_hash.clone(),
        };

        history.push_back(record);
        env.storage().instance().set(&agent_key, &history);
    }

    /// Helper: store immutable execution receipt (Issue #10)
    /// Receipts are stored separately and cannot be modified after creation
    fn store_execution_receipt(
        env: &Env,
        execution_id: u64,
        agent_id: u64,
        action: &String,
        executor: &Address,
        timestamp: u64,
        execution_hash: &Bytes,
    ) {
        let receipt_key = symbol_short!("receipt");
        let exec_receipt_key = (receipt_key, execution_id);

        // Create immutable receipt
        let receipt = ExecutionReceipt {
            execution_id,
            agent_id,
            action: action.clone(),
            executor: executor.clone(),
            timestamp,
            execution_hash: execution_hash.clone(),
            created_at: env.ledger().timestamp(),
        };

        // Store receipt - immutable after creation
        env.storage().instance().set(&exec_receipt_key, &receipt);

        // Map execution ID to agent for reverse lookups
        let exec_agent_key = symbol_short!("exagent");
        let exec_to_agent_key = (exec_agent_key, execution_id);
        env.storage().instance().set(&exec_to_agent_key, &agent_id);
    }

    // Helper: check rate limit (uses effective config; skips if bypass active)
    fn check_rate_limit(env: &Env, agent_id: u64) {
        if Self::has_active_bypass(env, agent_id) {
            return;
        }
        let config = Self::get_effective_rate_limit(env, agent_id);
        let max_operations = config.operations;
        let window_seconds = config.window_seconds;

        let now = env.ledger().timestamp();
        let limit_key = symbol_short!("ratelim");
        let agent_limit_key = (limit_key, agent_id);

        let rate_data: Option<RateLimitData> = env.storage().instance().get(&agent_limit_key);
        let (last_reset, count) = match rate_data {
            Some(data) => (data.last_reset, data.count),
            None => (now, 0),
        };

        let elapsed = now.saturating_sub(last_reset);

        let (new_reset, new_count) = if elapsed > window_seconds {
            (now, 1)
        } else if count < max_operations {
            (last_reset, count + 1)
        } else {
            panic!("Rate limit exceeded");
        };

        let new_rate_data = RateLimitData {
            last_reset: new_reset,
            count: new_count,
        };

        env.storage()
            .instance()
            .set(&agent_limit_key, &new_rate_data);
    }

    // Helper: Get agent owner from AgentNFT contract
    fn get_agent_owner(env: &Env, agent_id: u64) -> Address {
        let agent_nft_addr: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(env, AGENT_NFT_KEY))
            .expect("AgentNFT contract not set");

        // Call AgentNFT.get_agent_owner(agent_id)
        env.invoke_contract(
            &agent_nft_addr,
            &Symbol::new(env, "get_agent_owner"),
            Vec::from_array(env, [agent_id.into_val(env)]),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Ledger;
    use soroban_sdk::{testutils::Address as _, Env};

    // Mock AgentNFT contract for testing cross-contract calls
    #[contract]
    pub struct MockAgentNFT;

    #[contractimpl]
    impl MockAgentNFT {
        pub fn get_agent_owner(env: Env, agent_id: u64) -> Address {
            env.storage()
                .instance()
                .get(&agent_id)
                .expect("Agent not found in mock")
        }

        pub fn set_owner(env: Env, agent_id: u64, owner: Address) {
            env.storage().instance().set(&agent_id, &owner);
        }
    }

    fn setup_test() -> (
        Env,
        ExecutionHubClient<'static>,
        Address,
        MockAgentNFTClient<'static>,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, ExecutionHub);
        let client = ExecutionHubClient::new(&env, &contract_id);

        let agent_nft_id = env.register_contract(None, MockAgentNFT);
        let agent_nft_client = MockAgentNFTClient::new(&env, &agent_nft_id);

        let admin = Address::generate(&env);

        // Initialize with agent nft address
        client.initialize(&admin, &agent_nft_id);

        (env, client, admin, agent_nft_client, agent_nft_id)
    }

    #[test]
    fn test_initialization() {
        let (env, client, admin, _, agent_nft_id) = setup_test();

        assert_eq!(client.get_admin(), admin);
        assert_eq!(client.get_execution_counter(), 0);

        // Check if agent nft address is stored correctly is implicit via get_agent_owner working later
    }

    #[test]
    #[should_panic(expected = "Contract already initialized")]
    fn test_double_initialization() {
        let (env, client, admin, _, agent_nft_id) = setup_test();
        client.initialize(&admin, &agent_nft_id);
    }

    #[test]
    fn test_execution_counter_increment() {
        let (env, client, _admin, agent_nft, _) = setup_test();
        let executor = Address::generate(&env);

        // Set executor as owner of agent 1
        agent_nft.set_owner(&1, &executor);

        let action = String::from_str(&env, "test_action");
        let params = Bytes::from_array(&env, &[1, 2, 3]);
        let exec_hash = Bytes::from_array(&env, &[0xab, 0xcd, 0xef]);

        let exec_id_1 = client.execute_action(&1, &executor, &action, &params, &1, &exec_hash);
        assert_eq!(exec_id_1, 1);
        assert_eq!(client.get_execution_counter(), 1);

        let exec_hash_2 = Bytes::from_array(&env, &[0x12, 0x34, 0x56]);
        let exec_id_2 = client.execute_action(&1, &executor, &action, &params, &2, &exec_hash_2);
        assert_eq!(exec_id_2, 2);
        assert_eq!(client.get_execution_counter(), 2);
    }

    #[test]
    fn test_permission_checks() {
        let (env, client, _admin, agent_nft, _) = setup_test();
        let owner = Address::generate(&env);
        let other = Address::generate(&env);

        // Set owner for agent 1
        agent_nft.set_owner(&1, &owner);

        let action = String::from_str(&env, "test");
        let params = Bytes::from_array(&env, &[1]);
        let exec_hash = Bytes::from_array(&env, &[0xaa]);

        // 1. Owner can execute
        client.execute_action(&1, &owner, &action, &params, &1, &exec_hash);

        // 2. Non-owner cannot execute
        // We expect panic here. Since we can't easily catch panic in the middle of a test without helper,
        // we'll rely on separate tests or use verify_executed pattern if available.
        // For now, let's just test success cases and create a separate test for failure.
    }

    #[test]
    #[should_panic(expected = "Unauthorized: executor is not owner or operator")]
    fn test_unauthorized_execution() {
        let (env, client, _admin, agent_nft, _) = setup_test();
        let owner = Address::generate(&env);
        let other = Address::generate(&env);

        agent_nft.set_owner(&1, &owner);

        let action = String::from_str(&env, "test");
        let params = Bytes::from_array(&env, &[1]);
        let exec_hash = Bytes::from_array(&env, &[0xaa]);

        // Other tries to execute
        client.execute_action(&1, &other, &action, &params, &1, &exec_hash);
    }

    #[test]
    fn test_operator_delegation() {
        let (env, client, _admin, agent_nft, _) = setup_test();
        let owner = Address::generate(&env);
        let operator = Address::generate(&env);

        agent_nft.set_owner(&1, &owner);

        // Authorize operator for 100 seconds
        client.authorize_operator(&1, &owner, &operator, &100);

        let action = String::from_str(&env, "test");
        let params = Bytes::from_array(&env, &[1]);
        let exec_hash = Bytes::from_array(&env, &[0xaa]);

        // Operator executes
        client.execute_action(&1, &operator, &action, &params, &1, &exec_hash);

        // Revoke
        client.revoke_operator(&1, &owner);

        // Should fail now (need separate test for panic)
    }

    #[test]
    #[should_panic(expected = "Unauthorized: executor is not owner or operator")]
    fn test_revoked_operator() {
        let (env, client, _admin, agent_nft, _) = setup_test();
        let owner = Address::generate(&env);
        let operator = Address::generate(&env);

        agent_nft.set_owner(&1, &owner);
        client.authorize_operator(&1, &owner, &operator, &100);
        client.revoke_operator(&1, &owner);

        let action = String::from_str(&env, "test");
        let params = Bytes::from_array(&env, &[1]);
        let exec_hash = Bytes::from_array(&env, &[0xaa]);

        client.execute_action(&1, &operator, &action, &params, &1, &exec_hash);
    }

    #[test]
    #[should_panic(expected = "Unauthorized: operator authorization expired")]
    fn test_expired_operator() {
        let (env, client, _admin, agent_nft, _) = setup_test();
        let owner = Address::generate(&env);
        let operator = Address::generate(&env);

        agent_nft.set_owner(&1, &owner);

        // Authorize for 10 seconds
        client.authorize_operator(&1, &owner, &operator, &10);

        // Advance time by 20 seconds
        env.ledger().set_timestamp(env.ledger().timestamp() + 20);

        let action = String::from_str(&env, "test");
        let params = Bytes::from_array(&env, &[1]);
        let exec_hash = Bytes::from_array(&env, &[0xaa]);

        client.execute_action(&1, &operator, &action, &params, &1, &exec_hash);
    }

    #[test]
    fn test_register_and_get_rule() {
        let (env, client, _admin, _, _) = setup_test();
        let owner = Address::generate(&env);

        let rule_name = String::from_str(&env, "my_rule");
        let rule_data = Bytes::from_array(&env, &[10, 20, 30]);

        client.register_rule(&1, &owner, &rule_name, &rule_data);

        let retrieved = client.get_rule(&1, &rule_name);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), rule_data);
    }

    #[test]
    #[should_panic(expected = "Invalid nonce")]
    fn test_replay_protection() {
        let (env, client, _admin, agent_nft, _) = setup_test();
        let executor = Address::generate(&env);
        agent_nft.set_owner(&1, &executor);

        let action = String::from_str(&env, "test");
        let params = Bytes::from_array(&env, &[1]);
        let exec_hash = Bytes::from_array(&env, &[0xaa, 0xbb]);

        client.execute_action(&1, &executor, &action, &params, &1, &exec_hash);
        client.execute_action(&1, &executor, &action, &params, &1, &exec_hash);
    }

    #[test]
    fn test_get_history() {
        let (env, client, _admin, agent_nft, _) = setup_test();
        let executor = Address::generate(&env);
        agent_nft.set_owner(&1, &executor);

        let action = String::from_str(&env, "test_action");
        let params = Bytes::from_array(&env, &[1]);
        let exec_hash_1 = Bytes::from_array(&env, &[0x11, 0x22]);
        let exec_hash_2 = Bytes::from_array(&env, &[0x33, 0x44]);

        client.execute_action(&1, &executor, &action, &params, &1, &exec_hash_1);
        client.execute_action(&1, &executor, &action, &params, &2, &exec_hash_2);

        let history = client.get_history(&1, &10);
        assert_eq!(history.len(), 2);
        assert_eq!(client.get_action_count(&1), 2);
    }

    #[test]
    fn test_admin_transfer() {
        let (env, client, admin1, _, _) = setup_test();
        let admin2 = Address::generate(&env);

        client.transfer_admin(&admin1, &admin2);
        assert_eq!(client.get_admin(), admin2);
    }

    #[test]
    fn test_rate_limiting() {
        let (env, client, _admin, agent_nft, _) = setup_test();
        let executor = Address::generate(&env);
        agent_nft.set_owner(&1, &executor);

        let action = String::from_str(&env, "test");
        let params = Bytes::from_array(&env, &[1]);

        for i in 1..=10 {
            let exec_hash = Bytes::from_array(&env, &[i as u8, (i * 2) as u8]);
            client.execute_action(&1, &executor, &action, &params, &i, &exec_hash);
        }

        let exec_hash_11 = Bytes::from_array(&env, &[11, 22]);
        let result = client.execute_action(&1, &executor, &action, &params, &11, &exec_hash_11);
        assert!(result > 0);
    }

    // Issue #10: Tests for execution receipt functionality
    #[test]
    fn test_execution_receipt_storage() {
        let (env, client, _admin, agent_nft, _) = setup_test();
        let executor = Address::generate(&env);
        agent_nft.set_owner(&1, &executor);

        let action = String::from_str(&env, "transfer");
        let params = Bytes::from_array(&env, &[1, 2, 3]);
        let exec_hash = Bytes::from_array(&env, &[0xde, 0xad, 0xbe, 0xef]);

        let exec_id = client.execute_action(&1, &executor, &action, &params, &1, &exec_hash);

        // Verify receipt was stored
        let receipt = client.get_execution_receipt(&exec_id);
        assert!(receipt.is_some());

        let receipt = receipt.unwrap();
        assert_eq!(receipt.execution_id, exec_id);
        assert_eq!(receipt.agent_id, 1);
        assert_eq!(receipt.action, action);
        assert_eq!(receipt.executor, executor);
        assert_eq!(receipt.execution_hash, exec_hash);
    }

    #[test]
    fn test_get_agent_for_execution() {
        let (env, client, _admin, agent_nft, _) = setup_test();
        let executor = Address::generate(&env);
        agent_nft.set_owner(&42, &executor);

        let action = String::from_str(&env, "action");
        let params = Bytes::from_array(&env, &[1]);
        let exec_hash = Bytes::from_array(&env, &[0xca, 0xfe]);

        let exec_id = client.execute_action(&42, &executor, &action, &params, &1, &exec_hash);

        // Verify reverse lookup works
        let agent_id = client.get_agent_for_execution(&exec_id);
        assert!(agent_id.is_some());
        assert_eq!(agent_id.unwrap(), 42);
    }

    #[test]
    fn test_get_agent_receipts() {
        let (env, client, _admin, agent_nft, _) = setup_test();
        let executor = Address::generate(&env);
        agent_nft.set_owner(&1, &executor);

        let action = String::from_str(&env, "batch_action");
        let params = Bytes::from_array(&env, &[1]);

        // Execute multiple actions for the same agent
        for i in 1..=5u64 {
            let exec_hash = Bytes::from_array(&env, &[i as u8, (i * 10) as u8]);
            client.execute_action(&1, &executor, &action, &params, &i, &exec_hash);
        }

        // Get all receipts for agent
        let receipts = client.get_agent_receipts(&1, &10);
        assert_eq!(receipts.len(), 5);
    }

    #[test]
    fn test_receipt_immutability() {
        let (env, client, _admin, agent_nft, _) = setup_test();
        let executor = Address::generate(&env);
        agent_nft.set_owner(&1, &executor);

        let action = String::from_str(&env, "immutable_test");
        let params = Bytes::from_array(&env, &[1]);
        let exec_hash = Bytes::from_array(&env, &[0x11, 0x22, 0x33]);

        let exec_id = client.execute_action(&1, &executor, &action, &params, &1, &exec_hash);

        // Get receipt
        let receipt_1 = client.get_execution_receipt(&exec_id).unwrap();

        // Execute another action
        let exec_hash_2 = Bytes::from_array(&env, &[0x44, 0x55, 0x66]);
        client.execute_action(&1, &executor, &action, &params, &2, &exec_hash_2);

        // Original receipt should remain unchanged
        let receipt_2 = client.get_execution_receipt(&exec_id).unwrap();
        assert_eq!(receipt_1.execution_hash, receipt_2.execution_hash);
        assert_eq!(receipt_1.timestamp, receipt_2.timestamp);
    }

    // --- Rate limit configuration tests ---

    #[test]
    fn test_rate_limit_config_storage_and_retrieval() {
        let (env, client, admin, agent_nft, _) = setup_test();
        let executor = Address::generate(&env);
        agent_nft.set_owner(&1, &executor);

        // After init, get_rate_limit returns global default (100, 60 from lib)
        let config = client.get_rate_limit(&1);
        assert_eq!(config.operations, 100);
        assert_eq!(config.window_seconds, 60);

        // Set per-agent override
        client.set_agent_rate_limit(&admin, &1, &200, &120);
        let config = client.get_rate_limit(&1);
        assert_eq!(config.operations, 200);
        assert_eq!(config.window_seconds, 120);

        // Agent 2 has no override, so gets global
        let config2 = client.get_rate_limit(&2);
        assert_eq!(config2.operations, 100);
        assert_eq!(config2.window_seconds, 60);
    }

    #[test]
    fn test_global_rate_limit_change() {
        let (env, client, admin, agent_nft, _) = setup_test();
        agent_nft.set_owner(&1, &Address::generate(&env));

        client.set_global_rate_limit(&admin, &50, &300);
        let config = client.get_rate_limit(&1);
        assert_eq!(config.operations, 50);
        assert_eq!(config.window_seconds, 300);
    }

    #[test]
    fn test_agent_override_and_reset() {
        let (env, client, admin, agent_nft, _) = setup_test();
        agent_nft.set_owner(&1, &Address::generate(&env));

        client.set_agent_rate_limit(&admin, &1, &500, &3600);
        let config = client.get_rate_limit(&1);
        assert_eq!(config.operations, 500);
        assert_eq!(config.window_seconds, 3600);

        client.reset_agent_rate_limit(&admin, &1);
        let config = client.get_rate_limit(&1);
        assert_eq!(config.operations, 100);
        assert_eq!(config.window_seconds, 60);
    }

    #[test]
    fn test_multiple_rate_limit_levels() {
        let (env, client, admin, agent_nft, _) = setup_test();
        let exec1 = Address::generate(&env);
        let exec2 = Address::generate(&env);
        agent_nft.set_owner(&1, &exec1);
        agent_nft.set_owner(&2, &exec2);

        client.set_global_rate_limit(&admin, &10, &60);
        client.set_agent_rate_limit(&admin, &1, &1000, &3600); // high-frequency agent

        assert_eq!(client.get_rate_limit(&1).operations, 1000);
        assert_eq!(client.get_rate_limit(&2).operations, 10);
    }

    #[test]
    #[should_panic(expected = "Rate limit exceeded")]
    fn test_rate_limit_integration_with_execution() {
        let (env, client, admin, agent_nft, _) = setup_test();
        let executor = Address::generate(&env);
        agent_nft.set_owner(&1, &executor);

        client.set_global_rate_limit(&admin, &3, &60);
        let action = String::from_str(&env, "test");
        let params = Bytes::from_array(&env, &[1]);

        for i in 1..=3u64 {
            let h = Bytes::from_array(&env, &[i as u8, (i * 2) as u8]);
            client.execute_action(&1, &executor, &action, &params, &i, &h);
        }
        let h4 = Bytes::from_array(&env, &[4, 8]);
        client.execute_action(&1, &executor, &action, &params, &4, &h4);
    }

    #[test]
    #[should_panic(expected = "operations must be greater than 0")]
    fn test_rate_limit_zero_ops_panics() {
        let (env, client, admin, _, _) = setup_test();
        client.set_global_rate_limit(&admin, &0, &60);
    }

    #[test]
    #[should_panic(expected = "window_seconds must be greater than 0")]
    fn test_rate_limit_zero_window_panics() {
        let (env, client, admin, _, _) = setup_test();
        client.set_global_rate_limit(&admin, &100, &0);
    }

    #[test]
    fn test_rate_limit_max_window_values() {
        let (env, client, admin, agent_nft, _) = setup_test();
        agent_nft.set_owner(&1, &Address::generate(&env));
        client.set_global_rate_limit(&admin, &1, &u64::MAX);
        let config = client.get_rate_limit(&1);
        assert_eq!(config.operations, 1);
        assert_eq!(config.window_seconds, u64::MAX);
    }

    #[test]
    fn test_bypass_allows_over_limit() {
        let (env, client, admin, agent_nft, _) = setup_test();
        let executor = Address::generate(&env);
        agent_nft.set_owner(&1, &executor);

        client.set_global_rate_limit(&admin, &2, &60);
        let action = String::from_str(&env, "test");
        let params = Bytes::from_array(&env, &[1]);
        let reason = String::from_str(&env, "emergency maintenance");

        let now = env.ledger().timestamp();
        client.set_rate_limit_bypass(&admin, &1, &reason, &(now + 3600));

        for i in 1..=5u64 {
            let h = Bytes::from_array(&env, &[i as u8]);
            let id = client.execute_action(&1, &executor, &action, &params, &i, &h);
            assert!(id > 0);
        }
    }

    #[test]
    #[should_panic(expected = "Rate limit exceeded")]
    fn test_bypass_cleared_then_limit_applies() {
        let (env, client, admin, agent_nft, _) = setup_test();
        let executor = Address::generate(&env);
        agent_nft.set_owner(&1, &executor);

        client.set_global_rate_limit(&admin, &1, &60);
        let action = String::from_str(&env, "test");
        let params = Bytes::from_array(&env, &[1]);
        let reason = String::from_str(&env, "brief bypass");
        let now = env.ledger().timestamp();
        client.set_rate_limit_bypass(&admin, &1, &reason, &(now + 3600));
        let h1 = Bytes::from_array(&env, &[1]);
        client.execute_action(&1, &executor, &action, &params, &1, &h1);

        client.clear_rate_limit_bypass(&admin, &1);
        let h2 = Bytes::from_array(&env, &[2]);
        client.execute_action(&1, &executor, &action, &params, &2, &h2);
        let h3 = Bytes::from_array(&env, &[3]);
        client.execute_action(&1, &executor, &action, &params, &3, &h3);
    }

    #[test]
    #[should_panic(expected = "valid_until must be in the future")]
    fn test_bypass_valid_until_must_be_future() {
        let (env, client, admin, _, _) = setup_test();
        let reason = String::from_str(&env, "reason");
        let now = env.ledger().timestamp();
        client.set_rate_limit_bypass(&admin, &1, &reason, &now);
    }

    #[test]
    #[should_panic(expected = "Unauthorized: caller is not admin")]
    fn test_set_global_rate_limit_non_admin_panics() {
        let (env, client, _admin, _, _) = setup_test();
        let other = Address::generate(&env);
        client.set_global_rate_limit(&other, &50, &60);
    }
}
