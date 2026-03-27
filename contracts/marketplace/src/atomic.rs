use soroban_sdk::{contracttype, Env, String, Symbol, Val, Vec};

use stellai_lib::atomic::AtomicTransactionSupport;

#[derive(Clone)]
#[contracttype]
pub struct AtomicStepState {
    pub transaction_id: u64,
    pub step_id: u32,
    pub prepared: bool,
    pub executed: bool,
    pub result: Option<String>,
}

pub struct MarketplaceAtomicSupport;

impl AtomicTransactionSupport for MarketplaceAtomicSupport {
    fn prepare_step(
        env: &Env,
        transaction_id: u64,
        step_id: u32,
        function: &Symbol,
        _args: &Vec<Val>,
    ) -> bool {
        // Simplified implementation - just mark as prepared
        let state = AtomicStepState {
            transaction_id,
            step_id,
            prepared: true,
            executed: false,
            result: None,
        };

        let key = (Symbol::new(env, "atomic_step"), transaction_id, step_id);
        env.storage().instance().set(&key, &state);
        true
    }

    fn commit_step(
        env: &Env,
        transaction_id: u64,
        step_id: u32,
        function: &Symbol,
        _args: &Vec<Val>,
    ) -> Val {
        // Simplified implementation - just mark as executed and return success
        let key = (Symbol::new(env, "atomic_step"), transaction_id, step_id);
        if let Some(mut state) = env.storage().instance().get::<_, AtomicStepState>(&key) {
            state.executed = true;
            env.storage().instance().set(&key, &state);
        }

        true.into()
    }

    fn is_step_prepared(env: &Env, transaction_id: u64, step_id: u32) -> bool {
        let key = (Symbol::new(env, "atomic_step"), transaction_id, step_id);
        env.storage()
            .instance()
            .get::<_, AtomicStepState>(&key)
            .map(|state| state.prepared)
            .unwrap_or(false)
    }

    fn get_step_result(env: &Env, transaction_id: u64, step_id: u32) -> Option<Val> {
        let key = (Symbol::new(env, "atomic_step"), transaction_id, step_id);
        env.storage()
            .instance()
            .get::<_, AtomicStepState>(&key)
            .map(|_| true.into())
    }

    fn rollback_step(
        env: &Env,
        transaction_id: u64,
        step_id: u32,
        _rollback_function: &Symbol,
        _rollback_args: &Vec<Val>,
    ) -> bool {
        // Simplified implementation - just remove the step state
        let key = (Symbol::new(env, "atomic_step"), transaction_id, step_id);
        env.storage().instance().remove(&key);
        true
    }
}
