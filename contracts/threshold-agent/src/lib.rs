use soroban_sdk::{contract, contractimpl, Env, Address, Bytes, Symbol};
use crate::types::{ThresholdKeyShare, ThresholdProposal, ProposalStatus};

#[contract]
pub struct ThresholdAgentContract;

#[contractimpl]
impl ThresholdAgentContract {
    pub fn create_threshold_agent(env: Env, agent_id: u64, owners: Vec<Address>, threshold_m: u32) {
        assert!(threshold_m <= owners.len() as u32, "Threshold cannot exceed number of owners");
        for (i, owner) in owners.iter().enumerate() {
            let share = ThresholdKeyShare {
                agent_id,
                share_holder: owner.clone(),
                share_index: i as u32,
                x_coordinate: i as u32,
                y_coordinate_encrypted: Bytes::new(&env),
                commitment: Bytes::new(&env),
                created_at: env.ledger().timestamp(),
            };
            env.storage().persistent().set(&(agent_id, i as u32), &share);
        }
        env.events().publish((Symbol::new(&env, "ThresholdAgentCreated"), agent_id), owners);
    }

    pub fn propose_action(env: Env, agent_id: u64, action_data: Bytes, proposer: Address) -> u64 {
        let proposal_id = env.ledger().sequence();
        let proposal = ThresholdProposal {
            proposal_id,
            agent_id,
            action_data,
            proposer,
            threshold_m: 2, // Example threshold
            signatures: vec![],
            status: ProposalStatus::Pending,
        };
        env.storage().persistent().set(&(agent_id, proposal_id), &proposal);
        proposal_id
    }

    pub fn sign_proposal(env: Env, agent_id: u64, proposal_id: u64, signer: Address) {
        let mut proposal: ThresholdProposal = env.storage().persistent().get(&(agent_id, proposal_id)).unwrap();
        signer.require_auth();
        if !proposal.signatures.contains(&signer) {
            proposal.signatures.push(signer.clone());
            env.events().publish((Symbol::new(&env, "ProposalSigned"), proposal_id), signer);
        }
        if proposal.signatures.len() as u32 >= proposal.threshold_m {
            proposal.status = ProposalStatus::Executed;
            env.events().publish((Symbol::new(&env, "ThresholdActionExecuted"), proposal_id), proposal.action_data.clone());
        }
        env.storage().persistent().set(&(agent_id, proposal_id), &proposal);
    }

    pub fn get_threshold_status(env: Env, agent_id: u64, proposal_id: u64) -> ThresholdProposal {
        env.storage().persistent().get(&(agent_id, proposal_id)).unwrap()
    }
}
