#[cfg(test)]
mod prop_tests {
    use super::*;
    use crate::{AgentNFT, AgentNFTClient, ContractError};
    use proptest::prelude::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

    // --- Strategy Helpers ---
    // Generates a random valid royalty fee (0 to 10,000)
    fn any_royalty_fee() -> impl Strategy<Value = u32> {
        0..=10000u32
    }

    // Generates a vector of strings (capabilities) with length limits
    fn any_capabilities(
        env: &Env,
    ) -> impl Strategy<Value = core::primitive::vec::Vec<std::string::String>> {
        prop::collection::vec(".*", 0..10)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        #[test]
        fn prop_agent_counter_always_increases_correctly(num_mints in 1..50usize) {
            let env = Env::default();
            let contract_id = env.register_contract(None, AgentNFT);
            let client = AgentNFTClient::new(&env, &contract_id);
            let admin = Address::generate(&env);

            env.mock_all_auths();
            client.init_contract(&admin);

            let mut expected_counter = 0;
            for _ in 0..num_mints {
                let owner = Address::generate(&env);
                // Using legacy mint which utilizes the counter
                client.mint_agent_legacy(
                    &owner,
                    &String::from_str(&env, "Agent"),
                    &String::from_str(&env, "Hash"),
                    &Vec::new(&env),
                    &None,
                    &None
                );
                expected_counter += 1;

                // INVARIANT: Counter must match number of successful legacy mints
                prop_assert_eq!(client.total_agents().unwrap(), expected_counter);
            }
        }

        #[test]
        fn prop_royalty_fee_invariant(fee in 10001..u32::MAX) {
            let env = Env::default();
            let contract_id = env.register_contract(None, AgentNFT);
            let client = AgentNFTClient::new(&env, &contract_id);
            let admin = Address::generate(&env);

            env.mock_all_auths();
            client.init_contract(&admin);

            let owner = Address::generate(&env);
            let recipient = Address::generate(&env);

            // INVARIANT: Any fee > 10000 must return InvalidRoyaltyFee error
            let result = client.try_mint_agent(
                &1,
                &owner,
                &String::from_str(&env, "cid"),
                &1,
                &Some(recipient),
                &Some(fee)
            );

            match result {
                Err(Ok(ContractError::InvalidRoyaltyFee)) => {},
                _ => panic!("Should have failed with InvalidRoyaltyFee for value {}", fee),
            }
        }

        #[test]
        fn prop_transfer_auth_invariant(
            id in 1..100u64,
            random_user in prop::option::of(just(true)) // dummy for randomization
        ) {
            let env = Env::default();
            env.mock_all_auths();
            let (client, admin) = setup_contract(&env);

            let owner = Address::generate(&env);
            let stranger = Address::generate(&env);
            let _ = client.add_approved_minter(&admin, &owner);

            mint_test_agent(&env, &client, &owner, id as u128, "cid", 1);

            // INVARIANT: Only owner can transfer. Stranger must fail.
            // We force the 'stranger' to be the one calling require_auth via mock_all_auths logic
            let result = client.try_transfer_agent(&id, &stranger, &Address::generate(&env));

            match result {
                Err(Ok(ContractError::NotOwner)) => {},
                _ => panic!("Non-owner was able to initiate transfer or got wrong error"),
            }
        }
    }

    // --- Standard Unit Tests (Moving from lib.rs and adding more) ---

    #[test]
    fn test_get_agent_metadata() {
        let env = Env::default();
        let (client, admin) = setup_contract(&env);

        let owner = Address::generate(&env);
        client.add_approved_minter(&admin, &owner);

        let metadata_cid = "QmTestMetadataCID456";
        env.mock_all_auths();
        mint_test_agent(&env, &client, &owner, 2, metadata_cid, 5);

        // Test get_agent_metadata returns correct CID
        let result = client.get_agent_metadata(&2);
        assert_eq!(result, String::from_str(&env, metadata_cid));
    }

    #[test]
    fn test_get_agent_evolution_level() {
        let env = Env::default();
        let (client, admin) = setup_contract(&env);

        let owner = Address::generate(&env);
        client.add_approved_minter(&admin, &owner);

        let evolution_level = 7u32;
        env.mock_all_auths();
        mint_test_agent(&env, &client, &owner, 3, "QmEvolutionTest", evolution_level);

        // Test get_agent_evolution_level returns correct level
        let result = client.get_agent_evolution_level(&3);
        assert_eq!(result, evolution_level);
    }

    #[test]
    fn test_query_non_existent_agent() {
        let env = Env::default();
        let (client, _admin) = setup_contract(&env);

        // Try to query a non-existent agent - should return AgentNotFound
        let result = client.try_get_agent_owner(&999);
        assert!(result.is_err());

        let result = client.try_get_agent_metadata(&999);
        assert!(result.is_err());

        let result = client.try_get_agent_evolution_level(&999);
        assert!(result.is_err());
    }

    #[test]
    fn test_query_zero_agent_id() {
        let env = Env::default();
        let (client, _admin) = setup_contract(&env);

        // Query with agent_id = 0 should return InvalidAgentId
        let result = client.try_get_agent_owner(&0);
        assert!(result.is_err());

        let result = client.try_get_agent_metadata(&0);
        assert!(result.is_err());

        let result = client.try_get_agent_evolution_level(&0);
        assert!(result.is_err());
    }

    #[test]
    fn test_capabilities_limit_error() {
         let env = Env::default();
         let (client, admin) = setup_contract(&env);
         let owner = Address::generate(&env);
         client.add_approved_minter(&admin, &owner);

         // Max is usually 10 in these contracts
         let mut caps = Vec::new(&env);
         for _ in 0..15 {
             caps.push_back(String::from_str(&env, "cap"));
         }

         env.mock_all_auths();
         let result = client.try_mint_agent_legacy(
             &owner,
             &String::from_str(&env, "Name"),
             &String::from_str(&env, "Hash"),
             &caps,
             &None,
             &None
         );

         match result {
             Err(Ok(ContractError::CapabilitiesExceeded)) => {},
             _ => panic!("Should have failed with CapabilitiesExceeded, got {:?}", result),
         }
    }
}
