use soroban_sdk::{contracttype, Address, BytesN, Symbol, Val, Vec};

#[contracttype]
pub enum DataKey {
    Oracle(BytesN<32>),
    OracleNonce(BytesN<32>),
}

#[contracttype]
#[derive(Clone)]
pub struct RelayRequest {
    pub relay_contract: Address,
    pub oracle_pubkey: BytesN<32>,
    pub target_contract: Address,
    pub function: Symbol,
    pub args: Vec<Val>,
    pub nonce: u64,
    pub deadline: u64,
}
