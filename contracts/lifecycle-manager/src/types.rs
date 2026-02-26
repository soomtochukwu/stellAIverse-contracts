use soroban_sdk::contracttype;

#[contracttype]
pub enum DataLifecycle {
    Active,     // e.g. ongoing agents, active listings
    Historical, // completed requests, past transactions
    Archived,   // compressed/archived data
}
