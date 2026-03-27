#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, Env,
    Map, String, Symbol, Vec,
};
use stellai_lib::{audit, admin, validation};

// DID Document structure following W3C DID specification
#[derive(Clone, Debug)]
#[contracttype]
pub struct DIDDocument {
    pub did: String,
    pub controller: Address,
    pub verification_methods: Vec<VerificationMethod>,
    pub authentication: Vec<String>,
    pub assertion_method: Vec<String>,
    pub key_agreement: Vec<String>,
    pub capability_invocation: Vec<String>,
    pub capability_delegation: Vec<String>,
    pub service: Vec<Service>,
    pub created: u64,
    pub updated: u64,
    pub version_id: u64,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct VerificationMethod {
    pub id: String,
    pub type_: String,
    pub controller: String,
    pub public_key: Bytes,
    pub created: u64,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct Service {
    pub id: String,
    pub type_: String,
    pub service_endpoint: String,
    pub created: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum DIDStatus {
    Active = 0,
    Suspended = 1,
    Revoked = 2,
}

#[derive(Clone)]
#[contracttype]
pub struct DIDRecord {
    pub document: DIDDocument,
    pub status: DIDStatus,
    pub nonce: u64,
    pub last_activity: u64,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct DIDHistory {
    pub did: String,
    pub action: String,
    pub actor: Address,
    pub timestamp: u64,
    pub previous_version: Option<u64>,
    pub new_version: u64,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[contracttype]
#[repr(u32)]
pub enum VerificationMethodType {
    Ed25519VerificationKey2018 = 0,
    EcdsaSecp256k1VerificationKey2019 = 1,
    X25519KeyAgreementKey2019 = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[contracttype]
#[repr(u32)]
pub enum ServiceType {
    AgentRegistry = 0,
    CredentialRepository = 1,
    ComplianceService = 2,
    ReputationService = 3,
    Messaging = 4,
    Storage = 5,
}

// Contract errors
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidDIDFormat = 1,
    DIDAlreadyExists = 2,
    DIDNotFound = 3,
    Unauthorized = 4,
    InvalidVerificationMethod = 5,
    InvalidService = 6,
    MaxVerificationMethodsExceeded = 7,
    MaxServicesExceeded = 8,
    InvalidSignature = 9,
    DIDRevoked = 10,
    DIDSuspended = 11,
    InvalidController = 12,
    InvalidPublicKey = 13,
    UnsupportedKeyType = 14,
    RateLimitExceeded = 15,
    AuditRequired = 16,
}

// Contract events
#[contracttype]
pub enum DIDEvent {
    DIDCreated(DIDCreatedEvent),
    DIDUpdated(DIDUpdatedEvent),
    DIDSuspended(DIDSuspendedEvent),
    DIDRevoked(DIDRevokedEvent),
    DIDReactivated(DIDReactivatedEvent),
    VerificationMethodAdded(VerificationMethodAddedEvent),
    VerificationMethodRemoved(VerificationMethodRemovedEvent),
    ServiceAdded(ServiceAddedEvent),
    ServiceRemoved(ServiceRemovedEvent),
}

#[derive(Clone)]
#[contracttype]
pub struct DIDCreatedEvent {
    pub did: String,
    pub controller: Address,
    pub timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct DIDUpdatedEvent {
    pub did: String,
    pub version_id: u64,
    pub updated_by: Address,
    pub timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct DIDSuspendedEvent {
    pub did: String,
    pub suspended_by: Address,
    pub reason: String,
    pub timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct DIDRevokedEvent {
    pub did: String,
    pub revoked_by: Address,
    pub reason: String,
    pub timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct DIDReactivatedEvent {
    pub did: String,
    pub reactivated_by: Address,
    pub timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct VerificationMethodAddedEvent {
    pub did: String,
    pub method_id: String,
    pub method_type: String,
    pub added_by: Address,
    pub timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct VerificationMethodRemovedEvent {
    pub did: String,
    pub method_id: String,
    pub removed_by: Address,
    pub timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct ServiceAddedEvent {
    pub did: String,
    pub service_id: String,
    pub service_type: String,
    pub added_by: Address,
    pub timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct ServiceRemovedEvent {
    pub did: String,
    pub service_id: String,
    pub removed_by: Address,
    pub timestamp: u64,
}

// Storage keys
const DID_REGISTRY: Symbol = symbol_short!("did_reg");
const DID_HISTORY: Symbol = symbol_short!("did_hist");
const CONTROLLER_MAPPING: Symbol = symbol_short!("ctrl_map");
const NONCE_COUNTER: Symbol = symbol_short!("nonce_ct");
const RATE_LIMIT_TRACKER: Symbol = symbol_short!("rate_lim");

// Constants
const MAX_VERIFICATION_METHODS: u32 = 10;
const MAX_SERVICES: u32 = 20;
const DID_PREFIX: &str = "did:stellar:";
const MAX_DID_LENGTH: u32 = 100;
const MAX_HISTORY_SIZE: u32 = 1000;

pub struct DIDContract;

#[contractimpl]
impl DIDContract {
    /// Create a new DID document
    pub fn create_did(
        env: Env,
        controller: Address,
        verification_methods: Vec<VerificationMethod>,
        services: Vec<Service>,
    ) -> Result<String, Error> {
        // Validate inputs
        Self::validate_create_did_inputs(env.clone(), &controller, &verification_methods, &services)?;

        // Generate DID
        let did = Self::generate_did(env.clone(), &controller);
        
        // Create DID document
        let now = env.ledger().timestamp();
        let document = DIDDocument {
            did: did.clone(),
            controller: controller.clone(),
            verification_methods: verification_methods.clone(),
            authentication: verification_methods.iter().map(|vm| vm.id.clone()).collect(),
            assertion_method: Vec::new(&env),
            key_agreement: Vec::new(&env),
            capability_invocation: Vec::new(&env),
            capability_delegation: Vec::new(&env),
            service: services.clone(),
            created: now,
            updated: now,
            version_id: 1,
        };

        // Create DID record
        let record = DIDRecord {
            document,
            status: DIDStatus::Active,
            nonce: 1,
            last_activity: now,
        };

        // Store DID record
        env.storage().instance().set(&DID_REGISTRY, &did, &record);
        
        // Store controller mapping
        env.storage().instance().set(&CONTROLLER_MAPPING, &controller, &did);

        // Create history entry
        let history = DIDHistory {
            did: did.clone(),
            action: String::from_str(&env, "created"),
            actor: controller,
            timestamp: now,
            previous_version: None,
            new_version: 1,
            reason: None,
        };
        
        // Store history
        Self::add_to_history(env.clone(), did.clone(), history);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "DIDCreated"), &did),
            DIDCreatedEvent {
                did: did.clone(),
                controller,
                timestamp: now,
            },
        );

        // Audit log
        audit::log_action(
            env,
            "create_did",
            &did,
            &controller,
            now,
            None,
        );

        Ok(did)
    }

    /// Update DID document
    pub fn update_did(
        env: Env,
        did: String,
        controller: Address,
        verification_methods: Option<Vec<VerificationMethod>>,
        services: Option<Vec<Service>>,
    ) -> Result<u64, Error> {
        // Validate authorization
        Self::validate_did_ownership(env.clone(), &did, &controller)?;

        // Get current record
        let mut record = Self::get_did_record(env.clone(), &did)?;
        
        // Check if DID is active
        if record.status != DIDStatus::Active {
            return Err(Error::DIDRevoked);
        }

        // Update document
        let now = env.ledger().timestamp();
        let old_version = record.document.version_id;
        
        if let Some(new_vms) = verification_methods {
            if new_vms.len() > MAX_VERIFICATION_METHODS as usize {
                return Err(Error::MaxVerificationMethodsExceeded);
            }
            record.document.verification_methods = new_vms;
            record.document.authentication = record.document.verification_methods
                .iter()
                .map(|vm| vm.id.clone())
                .collect();
        }

        if let Some(new_services) = services {
            if new_services.len() > MAX_SERVICES as usize {
                return Err(Error::MaxServicesExceeded);
            }
            record.document.service = new_services;
        }

        record.document.updated = now;
        record.document.version_id += 1;
        record.last_activity = now;
        record.nonce += 1;

        // Store updated record
        env.storage().instance().set(&DID_REGISTRY, &did, &record);

        // Create history entry
        let history = DIDHistory {
            did: did.clone(),
            action: String::from_str(&env, "updated"),
            actor: controller,
            timestamp: now,
            previous_version: Some(old_version),
            new_version: record.document.version_id,
            reason: None,
        };
        
        // Store history
        Self::add_to_history(env.clone(), did.clone(), history);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "DIDUpdated"), &did),
            DIDUpdatedEvent {
                did: did.clone(),
                version_id: record.document.version_id,
                updated_by: controller,
                timestamp: now,
            },
        );

        // Audit log
        audit::log_action(
            env,
            "update_did",
            &did,
            &controller,
            now,
            None,
        );

        Ok(record.document.version_id)
    }

    /// Suspend a DID
    pub fn suspend_did(
        env: Env,
        did: String,
        admin: Address,
        reason: String,
    ) -> Result<(), Error> {
        // Validate admin authorization
        admin::require_admin(env.clone(), &admin)?;

        // Get current record
        let mut record = Self::get_did_record(env.clone(), &did)?;
        
        // Check if DID can be suspended
        if record.status == DIDStatus::Revoked {
            return Err(Error::DIDRevoked);
        }
        if record.status == DIDStatus::Suspended {
            return Err(Error::DIDSuspended);
        }

        // Update status
        let now = env.ledger().timestamp();
        record.status = DIDStatus::Suspended;
        record.last_activity = now;

        // Store updated record
        env.storage().instance().set(&DID_REGISTRY, &did, &record);

        // Create history entry
        let history = DIDHistory {
            did: did.clone(),
            action: String::from_str(&env, "suspended"),
            actor: admin,
            timestamp: now,
            previous_version: Some(record.document.version_id),
            new_version: record.document.version_id,
            reason: Some(reason.clone()),
        };
        
        // Store history
        Self::add_to_history(env.clone(), did.clone(), history);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "DIDSuspended"), &did),
            DIDSuspendedEvent {
                did: did.clone(),
                suspended_by: admin,
                reason: reason.clone(),
                timestamp: now,
            },
        );

        // Audit log
        audit::log_action(
            env,
            "suspend_did",
            &did,
            &admin,
            now,
            Some(reason),
        );

        Ok(())
    }

    /// Revoke a DID
    pub fn revoke_did(
        env: Env,
        did: String,
        admin: Address,
        reason: String,
    ) -> Result<(), Error> {
        // Validate admin authorization
        admin::require_admin(env.clone(), &admin)?;

        // Get current record
        let mut record = Self::get_did_record(env.clone(), &did)?;
        
        // Check if DID can be revoked
        if record.status == DIDStatus::Revoked {
            return Err(Error::DIDRevoked);
        }

        // Update status
        let now = env.ledger().timestamp();
        record.status = DIDStatus::Revoked;
        record.last_activity = now;

        // Store updated record
        env.storage().instance().set(&DID_REGISTRY, &did, &record);

        // Create history entry
        let history = DIDHistory {
            did: did.clone(),
            action: String::from_str(&env, "revoked"),
            actor: admin,
            timestamp: now,
            previous_version: Some(record.document.version_id),
            new_version: record.document.version_id,
            reason: Some(reason.clone()),
        };
        
        // Store history
        Self::add_to_history(env.clone(), did.clone(), history);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "DIDRevoked"), &did),
            DIDRevokedEvent {
                did: did.clone(),
                revoked_by: admin,
                reason: reason.clone(),
                timestamp: now,
            },
        );

        // Audit log
        audit::log_action(
            env,
            "revoke_did",
            &did,
            &admin,
            now,
            Some(reason),
        );

        Ok(())
    }

    /// Reactivate a suspended DID
    pub fn reactivate_did(
        env: Env,
        did: String,
        admin: Address,
    ) -> Result<(), Error> {
        // Validate admin authorization
        admin::require_admin(env.clone(), &admin)?;

        // Get current record
        let mut record = Self::get_did_record(env.clone(), &did)?;
        
        // Check if DID can be reactivated
        if record.status != DIDStatus::Suspended {
            return Err(Error::DIDSuspended);
        }

        // Update status
        let now = env.ledger().timestamp();
        record.status = DIDStatus::Active;
        record.last_activity = now;

        // Store updated record
        env.storage().instance().set(&DID_REGISTRY, &did, &record);

        // Create history entry
        let history = DIDHistory {
            did: did.clone(),
            action: String::from_str(&env, "reactivated"),
            actor: admin,
            timestamp: now,
            previous_version: Some(record.document.version_id),
            new_version: record.document.version_id,
            reason: None,
        };
        
        // Store history
        Self::add_to_history(env.clone(), did.clone(), history);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "DIDReactivated"), &did),
            DIDReactivatedEvent {
                did: did.clone(),
                reactivated_by: admin,
                timestamp: now,
            },
        );

        // Audit log
        audit::log_action(
            env,
            "reactivate_did",
            &did,
            &admin,
            now,
            None,
        );

        Ok(())
    }

    /// Get DID document
    pub fn get_did_document(env: Env, did: String) -> Result<DIDDocument, Error> {
        let record = Self::get_did_record(env, &did)?;
        Ok(record.document)
    }

    /// Get DID record with status
    pub fn get_did_record(env: Env, did: &String) -> Result<DIDRecord, Error> {
        let record: Option<DIDRecord> = env.storage().instance().get(&DID_REGISTRY, did);
        record.ok_or(Error::DIDNotFound)
    }

    /// Get DID by controller
    pub fn get_did_by_controller(env: Env, controller: Address) -> Result<String, Error> {
        let did: Option<String> = env.storage().instance().get(&CONTROLLER_MAPPING, &controller);
        did.ok_or(Error::DIDNotFound)
    }

    /// Get DID history
    pub fn get_did_history(env: Env, did: String, limit: u32) -> Result<Vec<DIDHistory>, Error> {
        let history_key = (DID_HISTORY, did.clone());
        let history: Option<Vec<DIDHistory>> = env.storage().instance().get(&history_key);
        
        match history {
            Some(h) => {
                let effective_limit = if limit > MAX_HISTORY_SIZE { MAX_HISTORY_SIZE } else { limit };
                let end = if h.len() > effective_limit as usize { effective_limit as usize } else { h.len() };
                Ok(h.slice(0, end))
            }
            None => Ok(Vec::new(&env)),
        }
    }

    /// Check if DID is valid and active
    pub fn is_valid_did(env: Env, did: String) -> Result<bool, Error> {
        let record = Self::get_did_record(env, &did)?;
        Ok(record.status == DIDStatus::Active)
    }

    // Helper functions
    fn validate_create_did_inputs(
        env: Env,
        controller: &Address,
        verification_methods: &Vec<VerificationMethod>,
        services: &Vec<Service>,
    ) -> Result<(), Error> {
        // Check if controller already has a DID
        if let Ok(_) = Self::get_did_by_controller(env.clone(), controller.clone()) {
            return Err(Error::DIDAlreadyExists);
        }

        // Validate verification methods
        if verification_methods.len() > MAX_VERIFICATION_METHODS as usize {
            return Err(Error::MaxVerificationMethodsExceeded);
        }

        for vm in verification_methods {
            Self::validate_verification_method(vm)?;
        }

        // Validate services
        if services.len() > MAX_SERVICES as usize {
            return Err(Error::MaxServicesExceeded);
        }

        for service in services {
            Self::validate_service(service)?;
        }

        Ok(())
    }

    fn validate_verification_method(vm: &VerificationMethod) -> Result<(), Error> {
        // Validate key type
        if !Self::is_supported_key_type(&vm.type_) {
            return Err(Error::UnsupportedKeyType);
        }

        // Validate public key
        if vm.public_key.is_empty() || vm.public_key.len() > 64 {
            return Err(Error::InvalidPublicKey);
        }

        Ok(())
    }

    fn validate_service(service: &Service) -> Result<(), Error> {
        // Basic validation for service
        if service.id.is_empty() || service.type_.is_empty() || service.service_endpoint.is_empty() {
            return Err(Error::InvalidService);
        }

        Ok(())
    }

    fn is_supported_key_type(key_type: &str) -> bool {
        key_type == "Ed25519VerificationKey2018" ||
        key_type == "EcdsaSecp256k1VerificationKey2019" ||
        key_type == "X25519KeyAgreementKey2019"
    }

    fn generate_did(env: Env, controller: &Address) -> String {
        let timestamp = env.ledger().timestamp();
        let controller_str = controller.to_string();
        let hash = env.crypto().sha256(&controller_str.into());
        let hash_hex = hex::encode(hash);
        let short_hash = &hash_hex[..16];
        format!("{}{}:{}", DID_PREFIX, controller_str, short_hash)
    }

    fn validate_did_ownership(env: Env, did: &String, controller: &Address) -> Result<(), Error> {
        let record = Self::get_did_record(env.clone(), did)?;
        if record.document.controller != *controller {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }

    fn add_to_history(env: Env, did: String, history: DIDHistory) {
        let history_key = (DID_HISTORY, did.clone());
        let mut history_list: Vec<DIDHistory> = env.storage().instance()
            .get(&history_key)
            .unwrap_or(Vec::new(&env));
        
        history_list.push_front(history);
        
        // Keep only recent history
        if history_list.len() > MAX_HISTORY_SIZE as usize {
            history_list.pop_back();
        }
        
        env.storage().instance().set(&history_key, &history_list);
    }
}

// Include tests
#[cfg(test)]
mod tests;
