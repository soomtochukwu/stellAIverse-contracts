#[test]
fn test_threshold_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let owner1 = Address::random(&env);
    let owner2 = Address::random(&env);
    let owner3 = Address::random(&env);

    ThresholdAgentContract::create_threshold_agent(env.clone(), 1, vec![owner1.clone(), owner2.clone(), owner3.clone()], 2);

    let proposal_id = ThresholdAgentContract::propose_action(env.clone(), 1, Bytes::from_array(&env, &[1,2,3]), owner1.clone());

    ThresholdAgentContract::sign_proposal(env.clone(), 1, proposal_id, owner1.clone());
    ThresholdAgentContract::sign_proposal(env.clone(), 1, proposal_id, owner2.clone());

    let status = ThresholdAgentContract::get_threshold_status(env.clone(), 1, proposal_id);
    assert_eq!(status.status, ProposalStatus::Executed);
}
