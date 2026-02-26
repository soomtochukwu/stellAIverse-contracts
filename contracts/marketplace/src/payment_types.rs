use soroban_sdk::{contracttype, Address, String, Vec};

/// Individual payment split detail.
#[contracttype]
#[derive(Clone)]
pub struct PaymentSplit {
    pub recipient: Address,
    pub amount: i128,
    pub label: String,
}

/// Output of royalty split calculation.
#[contracttype]
#[derive(Clone)]
pub struct RoyaltyPaymentSplit {
    pub agent_id: u64,
    pub transaction_id: u64,
    pub sale_price: i128,
    pub royalty_rate_bps: u32,
    pub platform_fee_bps: u32,
    pub splits: Vec<PaymentSplit>,
}

/// Status for recorded payments.
#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PaymentStatus {
    Pending = 0,
    Completed = 1,
    Failed = 2,
}

/// Immutable audit trail entry for every routed payment.
#[contracttype]
#[derive(Clone)]
pub struct PaymentRecord {
    pub payment_id: u64,
    pub transaction_id: u64,
    pub agent_id: u64,
    pub total_amount: i128,
    pub splits: Vec<(Address, i128, String)>,
    pub timestamp: u64,
    pub status: PaymentStatus,
}
