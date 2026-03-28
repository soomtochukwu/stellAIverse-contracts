use soroban_sdk::contractassert;
use soroban_sdk::contracttest;
use soroban_sdk::symbol_short;
use soroban_sdk::testutils::{Address as TestAddress, Ledger as TestLedger};
use soroban_sdk::Address;
use soroban_sdk::Bytes;
use soroban_sdk::Env;
use soroban_sdk::String;
use soroban_sdk::Vec;

use crate::{DIDContract, DIDDocument, DIDRecord, DIDStatus, Error, Service, VerificationMethod};

#[contracttest]
fn test_create_did_success() {
    let env = Env::new();
    let contract_id = env.register_contract(None, DIDContract);
    let client = DIDContractClient::new(&env, &contract_id);

    // Setup admin
    let admin = Address::generate(&env);
    env.storage()
        .instance()
        .set(&symbol_short!("admin"), &admin);

    // Create test addresses
    let controller = Address::generate(&env);

    // Create verification methods
    let vm1 = VerificationMethod {
        id: String::from_str(&env, "key-1"),
        type_: String::from_str(&env, "Ed25519VerificationKey2018"),
        controller: String::from_str(&env, "did:stellar:test"),
        public_key: Bytes::from_array(&env, &[1u8; 32]),
        created: env.ledger().timestamp(),
    };

    let verification_methods = Vec::from_array(&env, [vm1]);

    // Create services
    let service1 = Service {
        id: String::from_str(&env, "service-1"),
        type_: String::from_str(&env, "AgentRegistry"),
        service_endpoint: String::from_str(&env, "https://api.stellai.verse/agents"),
        created: env.ledger().timestamp(),
    };

    let services = Vec::from_array(&env, [service1]);

    // Create DID
    let did = client.create_did(&controller, &verification_methods, &services);

    // Verify DID was created
    contractassert!(did.len() > 0);
    contractassert!(did.starts_with("did:stellar:"));

    // Verify DID document
    let document = client.get_did_document(&did);
    contractassert!(document.controller == controller);
    contractassert!(document.verification_methods.len() == 1);
    contractassert!(document.service.len() == 1);
    contractassert!(document.version_id == 1);

    // Verify DID record
    let record = client.get_did_record(&did);
    contractassert!(record.status == DIDStatus::Active);
    contractassert!(record.document.controller == controller);
}

#[contracttest]
fn test_create_did_already_exists() {
    let env = Env::new();
    let contract_id = env.register_contract(None, DIDContract);
    let client = DIDContractClient::new(&env, &contract_id);

    // Setup admin
    let admin = Address::generate(&env);
    env.storage()
        .instance()
        .set(&symbol_short!("admin"), &admin);

    // Create test addresses
    let controller = Address::generate(&env);

    // Create verification methods
    let vm1 = VerificationMethod {
        id: String::from_str(&env, "key-1"),
        type_: String::from_str(&env, "Ed25519VerificationKey2018"),
        controller: String::from_str(&env, "did:stellar:test"),
        public_key: Bytes::from_array(&env, &[1u8; 32]),
        created: env.ledger().timestamp(),
    };

    let verification_methods = Vec::from_array(&env, [vm1]);
    let services = Vec::new(&env);

    // Create first DID
    let did1 = client.create_did(&controller, &verification_methods, &services);

    // Try to create another DID for same controller - should fail
    let result = client.try_create_did(&controller, &verification_methods, &services);
    contractassert!(result.is_err());
    contractassert!(result.unwrap_err() == RawError::from_contract_error(Error::DIDAlreadyExists));
}

#[contracttest]
fn test_update_did_success() {
    let env = Env::new();
    let contract_id = env.register_contract(None, DIDContract);
    let client = DIDContractClient::new(&env, &contract_id);

    // Setup admin
    let admin = Address::generate(&env);
    env.storage()
        .instance()
        .set(&symbol_short!("admin"), &admin);

    // Create test addresses
    let controller = Address::generate(&env);

    // Create verification methods
    let vm1 = VerificationMethod {
        id: String::from_str(&env, "key-1"),
        type_: String::from_str(&env, "Ed25519VerificationKey2018"),
        controller: String::from_str(&env, "did:stellar:test"),
        public_key: Bytes::from_array(&env, &[1u8; 32]),
        created: env.ledger().timestamp(),
    };

    let verification_methods = Vec::from_array(&env, [vm1]);
    let services = Vec::new(&env);

    // Create DID
    let did = client.create_did(&controller, &verification_methods, &services);

    // Create new verification method
    let vm2 = VerificationMethod {
        id: String::from_str(&env, "key-2"),
        type_: String::from_str(&env, "Ed25519VerificationKey2018"),
        controller: String::from_str(&env, "did:stellar:test"),
        public_key: Bytes::from_array(&env, &[2u8; 32]),
        created: env.ledger().timestamp(),
    };

    let new_verification_methods = Vec::from_array(&env, [vm2]);

    // Update DID
    let new_version = client.update_did(&did, &controller, Some(&new_verification_methods), None);

    // Verify update
    contractassert!(new_version == 2);

    let document = client.get_did_document(&did);
    contractassert!(document.version_id == 2);
    contractassert!(document.verification_methods.len() == 1);
    contractassert!(
        document.verification_methods.get(0).unwrap().id == String::from_str(&env, "key-2")
    );
}

#[contracttest]
fn test_update_did_unauthorized() {
    let env = Env::new();
    let contract_id = env.register_contract(None, DIDContract);
    let client = DIDContractClient::new(&env, &contract_id);

    // Setup admin
    let admin = Address::generate(&env);
    env.storage()
        .instance()
        .set(&symbol_short!("admin"), &admin);

    // Create test addresses
    let controller = Address::generate(&env);
    let unauthorized = Address::generate(&env);

    // Create verification methods
    let vm1 = VerificationMethod {
        id: String::from_str(&env, "key-1"),
        type_: String::from_str(&env, "Ed25519VerificationKey2018"),
        controller: String::from_str(&env, "did:stellar:test"),
        public_key: Bytes::from_array(&env, &[1u8; 32]),
        created: env.ledger().timestamp(),
    };

    let verification_methods = Vec::from_array(&env, [vm1]);
    let services = Vec::new(&env);

    // Create DID
    let did = client.create_did(&controller, &verification_methods, &services);

    // Try to update DID with unauthorized address - should fail
    let result = client.try_update_did(&did, &unauthorized, None, None);
    contractassert!(result.is_err());
    contractassert!(result.unwrap_err() == RawError::from_contract_error(Error::Unauthorized));
}

#[contracttest]
fn test_suspend_did_success() {
    let env = Env::new();
    let contract_id = env.register_contract(None, DIDContract);
    let client = DIDContractClient::new(&env, &contract_id);

    // Setup admin
    let admin = Address::generate(&env);
    env.storage()
        .instance()
        .set(&symbol_short!("admin"), &admin);

    // Create test addresses
    let controller = Address::generate(&env);

    // Create verification methods
    let vm1 = VerificationMethod {
        id: String::from_str(&env, "key-1"),
        type_: String::from_str(&env, "Ed25519VerificationKey2018"),
        controller: String::from_str(&env, "did:stellar:test"),
        public_key: Bytes::from_array(&env, &[1u8; 32]),
        created: env.ledger().timestamp(),
    };

    let verification_methods = Vec::from_array(&env, [vm1]);
    let services = Vec::new(&env);

    // Create DID
    let did = client.create_did(&controller, &verification_methods, &services);

    // Suspend DID
    client.suspend_did(
        &did,
        &admin,
        &String::from_str(&env, "Suspension for investigation"),
    );

    // Verify suspension
    let record = client.get_did_record(&did);
    contractassert!(record.status == DIDStatus::Suspended);
}

#[contracttest]
fn test_revoke_did_success() {
    let env = Env::new();
    let contract_id = env.register_contract(None, DIDContract);
    let client = DIDContractClient::new(&env, &contract_id);

    // Setup admin
    let admin = Address::generate(&env);
    env.storage()
        .instance()
        .set(&symbol_short!("admin"), &admin);

    // Create test addresses
    let controller = Address::generate(&env);

    // Create verification methods
    let vm1 = VerificationMethod {
        id: String::from_str(&env, "key-1"),
        type_: String::from_str(&env, "Ed25519VerificationKey2018"),
        controller: String::from_str(&env, "did:stellar:test"),
        public_key: Bytes::from_array(&env, &[1u8; 32]),
        created: env.ledger().timestamp(),
    };

    let verification_methods = Vec::from_array(&env, [vm1]);
    let services = Vec::new(&env);

    // Create DID
    let did = client.create_did(&controller, &verification_methods, &services);

    // Revoke DID
    client.revoke_did(
        &did,
        &admin,
        &String::from_str(&env, "Revocation for policy violation"),
    );

    // Verify revocation
    let record = client.get_did_record(&did);
    contractassert!(record.status == DIDStatus::Revoked);
}

#[contracttest]
fn test_reactivate_did_success() {
    let env = Env::new();
    let contract_id = env.register_contract(None, DIDContract);
    let client = DIDContractClient::new(&env, &contract_id);

    // Setup admin
    let admin = Address::generate(&env);
    env.storage()
        .instance()
        .set(&symbol_short!("admin"), &admin);

    // Create test addresses
    let controller = Address::generate(&env);

    // Create verification methods
    let vm1 = VerificationMethod {
        id: String::from_str(&env, "key-1"),
        type_: String::from_str(&env, "Ed25519VerificationKey2018"),
        controller: String::from_str(&env, "did:stellar:test"),
        public_key: Bytes::from_array(&env, &[1u8; 32]),
        created: env.ledger().timestamp(),
    };

    let verification_methods = Vec::from_array(&env, [vm1]);
    let services = Vec::new(&env);

    // Create DID
    let did = client.create_did(&controller, &verification_methods, &services);

    // Suspend DID
    client.suspend_did(
        &did,
        &admin,
        &String::from_str(&env, "Suspension for investigation"),
    );

    // Reactivate DID
    client.reactivate_did(&did, &admin);

    // Verify reactivation
    let record = client.get_did_record(&did);
    contractassert!(record.status == DIDStatus::Active);
}

#[contracttest]
fn test_get_did_by_controller() {
    let env = Env::new();
    let contract_id = env.register_contract(None, DIDContract);
    let client = DIDContractClient::new(&env, &contract_id);

    // Setup admin
    let admin = Address::generate(&env);
    env.storage()
        .instance()
        .set(&symbol_short!("admin"), &admin);

    // Create test addresses
    let controller = Address::generate(&env);

    // Create verification methods
    let vm1 = VerificationMethod {
        id: String::from_str(&env, "key-1"),
        type_: String::from_str(&env, "Ed25519VerificationKey2018"),
        controller: String::from_str(&env, "did:stellar:test"),
        public_key: Bytes::from_array(&env, &[1u8; 32]),
        created: env.ledger().timestamp(),
    };

    let verification_methods = Vec::from_array(&env, [vm1]);
    let services = Vec::new(&env);

    // Create DID
    let did = client.create_did(&controller, &verification_methods, &services);

    // Get DID by controller
    let retrieved_did = client.get_did_by_controller(&controller);
    contractassert!(retrieved_did == did);
}

#[contracttest]
fn test_is_valid_did() {
    let env = Env::new();
    let contract_id = env.register_contract(None, DIDContract);
    let client = DIDContractClient::new(&env, &contract_id);

    // Setup admin
    let admin = Address::generate(&env);
    env.storage()
        .instance()
        .set(&symbol_short!("admin"), &admin);

    // Create test addresses
    let controller = Address::generate(&env);

    // Create verification methods
    let vm1 = VerificationMethod {
        id: String::from_str(&env, "key-1"),
        type_: String::from_str(&env, "Ed25519VerificationKey2018"),
        controller: String::from_str(&env, "did:stellar:test"),
        public_key: Bytes::from_array(&env, &[1u8; 32]),
        created: env.ledger().timestamp(),
    };

    let verification_methods = Vec::from_array(&env, [vm1]);
    let services = Vec::new(&env);

    // Create DID
    let did = client.create_did(&controller, &verification_methods, &services);

    // Check if DID is valid
    let is_valid = client.is_valid_did(&did);
    contractassert!(is_valid);

    // Suspend DID and check validity
    client.suspend_did(&did, &admin, &String::from_str(&env, "Test suspension"));

    let is_valid_after_suspension = client.is_valid_did(&did);
    contractassert!(!is_valid_after_suspension);
}

#[contracttest]
fn test_get_did_history() {
    let env = Env::new();
    let contract_id = env.register_contract(None, DIDContract);
    let client = DIDContractClient::new(&env, &contract_id);

    // Setup admin
    let admin = Address::generate(&env);
    env.storage()
        .instance()
        .set(&symbol_short!("admin"), &admin);

    // Create test addresses
    let controller = Address::generate(&env);

    // Create verification methods
    let vm1 = VerificationMethod {
        id: String::from_str(&env, "key-1"),
        type_: String::from_str(&env, "Ed25519VerificationKey2018"),
        controller: String::from_str(&env, "did:stellar:test"),
        public_key: Bytes::from_array(&env, &[1u8; 32]),
        created: env.ledger().timestamp(),
    };

    let verification_methods = Vec::from_array(&env, [vm1]);
    let services = Vec::new(&env);

    // Create DID
    let did = client.create_did(&controller, &verification_methods, &services);

    // Get history
    let history = client.get_did_history(&did, 10);
    contractassert!(history.len() == 1);
    contractassert!(history.get(0).unwrap().action == String::from_str(&env, "created"));
}
