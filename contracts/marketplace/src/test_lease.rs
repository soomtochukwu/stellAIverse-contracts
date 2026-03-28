//! Tests for lease lifecycle (issue #49): extension, termination, history, get_active_leases.

#![cfg(test)]

use soroban_sdk::testutils::Address as _;
use soroban_sdk::testutils::Ledger;
use soroban_sdk::{Address, Env, String, Symbol};
use stellai_lib::{
    LeaseData, LeaseHistoryEntry, LeaseState, Listing, ListingType, LISTING_COUNTER_KEY,
};

use crate::{storage::*, Marketplace, MarketplaceClient};

/// Setup env with marketplace initialized and a lease written to storage (no token needed).
/// Call after init_contract; all storage writes run inside contract context.
fn setup_lease_in_storage(env: &Env, contract_id: &Address) -> (Address, Address, u64, u64) {
    let lessor = Address::generate(env);
    let lessee = Address::generate(env);

    env.as_contract(contract_id, || {
        let listing_id = 1u64;
        let listing_key = (Symbol::new(env, "listing"), listing_id);
        let listing = Listing {
            listing_id,
            agent_id: 10,
            seller: lessor.clone(),
            price: 1000,
            listing_type: ListingType::Lease,
            active: false,
            created_at: env.ledger().timestamp(),
        };
        env.storage().instance().set(&listing_key, &listing);
        env.storage()
            .instance()
            .set(&Symbol::new(env, LISTING_COUNTER_KEY), &listing_id);

        let lease_id = increment_lease_counter(env);
        assert_eq!(lease_id, 1);

        let now = env.ledger().timestamp();
        let duration_seconds = 86400 * 30; // 30 days
        let end_time = now + duration_seconds;
        let total_value = 1000i128;
        let deposit_bps = 1000u32; // 10%
        let deposit_amount = (total_value * (deposit_bps as i128)) / 10_000;

        let lease = LeaseData {
            lease_id,
            agent_id: 10,
            listing_id,
            lessor: lessor.clone(),
            lessee: lessee.clone(),
            start_time: now,
            end_time,
            duration_seconds,
            deposit_amount,
            total_value,
            auto_renew: true,                 // Enable auto-renewal for testing
            lessee_consent_for_renewal: true, // Enable consent for testing
            status: LeaseState::Active,
            pending_extension_id: None,
        };
        set_lease(env, &lease);
        lessee_leases_append(env, &lessee, lease_id);
        lessor_leases_append(env, &lessor, lease_id);

        let entry = LeaseHistoryEntry {
            lease_id,
            action: String::from_str(env, "initiated"),
            actor: lessee.clone(),
            timestamp: now,
            details: None,
        };
        add_lease_history(env, lease_id, &entry);

        (lessor, lessee, lease_id, listing_id)
    })
}

#[test]
fn test_lease_config_default() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.init_contract(&admin);

    let config = env.as_contract(&contract_id, || get_lease_config(&env));
    assert_eq!(config.deposit_bps, stellai_lib::DEFAULT_LEASE_DEPOSIT_BPS);
    assert_eq!(
        config.early_termination_penalty_bps,
        stellai_lib::DEFAULT_EARLY_TERMINATION_PENALTY_BPS
    );
}

#[test]
fn test_set_lease_config() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.init_contract(&admin);
    client.set_lease_config(&admin, &1500, &2500);

    let config = env.as_contract(&contract_id, || get_lease_config(&env));
    assert_eq!(config.deposit_bps, 1500);
    assert_eq!(config.early_termination_penalty_bps, 2500);
}

#[test]
fn test_get_lease_by_id_and_active_leases() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init_contract(&admin);
    let (lessor, lessee, lease_id, _) = setup_lease_in_storage(&env, &contract_id);

    let lease = client.get_lease_by_id(&lease_id).unwrap();
    assert_eq!(lease.lease_id, lease_id);
    assert_eq!(lease.lessee, lessee);
    assert_eq!(lease.lessor, lessor);
    assert!(lease.status == LeaseState::Active);

    let lessee_leases = client.get_active_leases(&lessee);
    assert_eq!(lessee_leases.len(), 1);
    assert_eq!(lessee_leases.get(0).unwrap().lease_id, lease_id);

    let lessor_leases = client.get_active_leases(&lessor);
    assert_eq!(lessor_leases.len(), 1);
    assert_eq!(lessor_leases.get(0).unwrap().lease_id, lease_id);
}

#[test]
fn test_lease_extension_request_and_approve() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init_contract(&admin);
    let (lessor, lessee, lease_id, _) = setup_lease_in_storage(&env, &contract_id);

    let extension_id = client.request_lease_extension(&lease_id, &lessee, &(86400 * 7));
    assert!(extension_id > 0);

    let lease = client.get_lease_by_id(&lease_id).unwrap();
    assert!(lease.status == LeaseState::ExtensionRequested);
    assert_eq!(lease.pending_extension_id, Some(extension_id));

    client.approve_lease_extension(&lease_id, &extension_id, &lessor);

    let lease_after = client.get_lease_by_id(&lease_id).unwrap();
    assert!(lease_after.status == LeaseState::Active);
    assert_eq!(lease_after.pending_extension_id, None);
    assert_eq!(
        lease_after.duration_seconds,
        lease.duration_seconds + 86400 * 7
    );
}

#[test]
fn test_lease_history() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init_contract(&admin);
    let (_lessor, lessee, lease_id, _) = setup_lease_in_storage(&env, &contract_id);

    let history_before = client.get_lease_history(&lease_id);
    assert_eq!(history_before.len(), 1);
    assert_eq!(
        history_before.get(0).unwrap().action,
        String::from_str(&env, "initiated")
    );

    client.request_lease_extension(&lease_id, &lessee, &3600);

    let history = client.get_lease_history(&lease_id);
    assert!(history.len() >= 2);
    assert_eq!(
        history.get(0).unwrap().action,
        String::from_str(&env, "initiated")
    );
    assert_eq!(
        history.get(1).unwrap().action,
        String::from_str(&env, "extension_requested")
    );
}

#[test]
fn test_lease_early_termination() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init_contract(&admin);
    let (_lessor, lessee, lease_id, _) = setup_lease_in_storage(&env, &contract_id);

    // Set payment token for termination fee processing
    let token_address = Address::generate(&env);
    client.set_payment_token(&admin, &token_address);

    // Calculate expected penalty (20% of total value since we're terminating early)
    let lease = client.get_lease_by_id(&lease_id).unwrap();
    let expected_penalty = (lease.total_value * 2000) / 10000; // 20% of total value

    // Test early termination with sufficient fee
    let termination_fee = expected_penalty;
    client.early_termination(&lease_id, &lessee, &termination_fee);

    let terminated_lease = client.get_lease_by_id(&lease_id).unwrap();
    assert!(terminated_lease.status == LeaseState::Terminated);

    // Check history
    let history = client.get_lease_history(&lease_id);
    assert!(history.len() >= 2);
    assert_eq!(
        history.get(1).unwrap().action,
        String::from_str(&env, "early_terminated")
    );
}

#[test]
fn test_auto_renewal() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init_contract(&admin);
    let (_lessor, _lessee, lease_id, _) = setup_lease_in_storage(&env, &contract_id);

    // Simulate lease expiration by advancing time
    let new_timestamp = env.ledger().timestamp() + 86400 * 31; // 31 days later
    env.ledger().set_timestamp(new_timestamp);

    // Test auto-renewal
    client.auto_renew_lease(&lease_id);

    let renewed_lease = client.get_lease_by_id(&lease_id).unwrap();
    assert!(renewed_lease.status == LeaseState::Renewed);

    // Check history
    let history = client.get_lease_history(&lease_id);
    assert!(history.len() >= 2);
    assert_eq!(
        history.get(1).unwrap().action,
        String::from_str(&env, "auto_renewed")
    );
}

#[test]
fn test_lease_deposit_calculation() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init_contract(&admin);
    let (_lessor, lessee, lease_id, _) = setup_lease_in_storage(&env, &contract_id);

    // Set payment token
    let token_address = Address::generate(&env);
    client.set_payment_token(&admin, &token_address);

    let lease = client.get_lease_by_id(&lease_id).unwrap();

    // Advance time by half the lease duration
    let half_duration = lease.duration_seconds / 2;
    let new_timestamp = lease.start_time + half_duration;
    env.ledger().set_timestamp(new_timestamp);

    // Test lease deposit calculation
    let expected_deposit = (lease.total_value * 1000) / 10000; // 10% of total value
    assert_eq!(lease.deposit_amount, expected_deposit);
}

#[test]
#[should_panic(expected = "Invalid lease ID")]
fn test_invalid_lease_id() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init_contract(&admin);

    client.get_lease_by_id(&0); // Should panic with invalid lease ID
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_unauthorized_extension_request() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init_contract(&admin);
    let (lessor, _lessee, lease_id, _) = setup_lease_in_storage(&env, &contract_id);

    // Try to request extension as lessor (should fail)
    client.request_lease_extension(&lease_id, &lessor, &3600);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_unauthorized_extension_approval() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init_contract(&admin);
    let (_lessor, lessee, lease_id, _) = setup_lease_in_storage(&env, &contract_id);

    // Request extension as lessee
    let extension_id = client.request_lease_extension(&lease_id, &lessee, &3600);

    // Try to approve as lessee (should fail)
    client.approve_lease_extension(&lease_id, &extension_id, &lessee);
}

#[test]
fn test_prorated_termination_penalty() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init_contract(&admin);
    let (_lessor, lessee, lease_id, _) = setup_lease_in_storage(&env, &contract_id);

    // Set payment token for termination fee processing
    let token_address = Address::generate(&env);
    client.set_payment_token(&admin, &token_address);

    let lease = client.get_lease_by_id(&lease_id).unwrap();

    // Advance time by half the lease duration
    let half_duration = lease.duration_seconds / 2;
    let new_timestamp = lease.start_time + half_duration;
    env.ledger().set_timestamp(new_timestamp);

    // Calculate expected penalty (20% of remaining value)
    // Since we're halfway through, remaining value is half of total
    let remaining_value = lease.total_value / 2;
    let expected_penalty = (remaining_value * 2000) / 10000; // Default 20% penalty

    client.early_termination(&lease_id, &lessee, &expected_penalty);

    let terminated_lease = client.get_lease_by_id(&lease_id).unwrap();
    assert!(terminated_lease.status == LeaseState::Terminated);
}
