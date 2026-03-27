# Identity DID Contract

A decentralized identity (DID) management contract for StellAIverse, implementing W3C DID standards for on-chain identity registration and management.

## Overview

The Identity DID contract provides:
- DID registration and management following W3C DID specification
- Verification method management (Ed25519, ECDSA, X25519)
- Service endpoint registration
- DID lifecycle management (active, suspended, revoked)
- Comprehensive audit trail and event emissions

## Features

### Core Functionality
- **DID Registration**: Create new DIDs with verification methods and services
- **DID Management**: Update, suspend, revoke, and reactivate DIDs
- **Verification Methods**: Support for multiple cryptographic key types
- **Service Endpoints**: Register and manage DID services
- **History Tracking**: Complete audit trail of all DID operations

### Security Features
- **Authorization**: Only DID controllers can modify their DIDs
- **Admin Controls**: Admin-only suspension and revocation capabilities
- **Rate Limiting**: Protection against abuse
- **Audit Logging**: Complete operation tracking

## Contract Architecture

### Data Structures

#### DIDDocument
```rust
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
```

#### VerificationMethod
```rust
pub struct VerificationMethod {
    pub id: String,
    pub type_: String,
    pub controller: String,
    pub public_key: Bytes,
    pub created: u64,
}
```

#### Service
```rust
pub struct Service {
    pub id: String,
    pub type_: String,
    pub service_endpoint: String,
    pub created: u64,
}
```

### DID Status Management
- **Active**: Normal operational state
- **Suspended**: Temporarily disabled by admin
- **Revoked**: Permanently disabled

## API Reference

### Core Functions

#### create_did
```rust
pub fn create_did(
    env: Env,
    controller: Address,
    verification_methods: Vec<VerificationMethod>,
    services: Vec<Service>,
) -> Result<String, Error>
```
Creates a new DID document.

**Parameters:**
- `controller`: Address that controls the DID
- `verification_methods`: List of verification methods
- `services`: List of service endpoints

**Returns:** DID string identifier

#### update_did
```rust
pub fn update_did(
    env: Env,
    did: String,
    controller: Address,
    verification_methods: Option<Vec<VerificationMethod>>,
    services: Option<Vec<Service>>,
) -> Result<u64, Error>
```
Updates an existing DID document.

**Returns:** New version ID

#### suspend_did
```rust
pub fn suspend_did(
    env: Env,
    did: String,
    admin: Address,
    reason: String,
) -> Result<(), Error>
```
Suspends a DID (admin only).

#### revoke_did
```rust
pub fn revoke_did(
    env: Env,
    did: String,
    admin: Address,
    reason: String,
) -> Result<(), Error>
```
Revokes a DID (admin only).

#### reactivate_did
```rust
pub fn reactivate_did(
    env: Env,
    did: String,
    admin: Address,
) -> Result<(), Error>
```
Reactivates a suspended DID (admin only).

### Query Functions

#### get_did_document
```rust
pub fn get_did_document(env: Env, did: String) -> Result<DIDDocument, Error>
```
Retrieves the DID document.

#### get_did_record
```rust
pub fn get_did_record(env: Env, did: &String) -> Result<DIDRecord, Error>
```
Retrieves the full DID record with status.

#### get_did_by_controller
```rust
pub fn get_did_by_controller(env: Env, controller: Address) -> Result<String, Error>
```
Finds DID by controller address.

#### get_did_history
```rust
pub fn get_did_history(env: Env, did: String, limit: u32) -> Result<Vec<DIDHistory>, Error>
```
Retrieves DID operation history.

#### is_valid_did
```rust
pub fn is_valid_did(env: Env, did: String) -> Result<bool, Error>
```
Checks if DID exists and is active.

## Events

The contract emits comprehensive events for all operations:

- **DIDCreated**: New DID registration
- **DIDUpdated**: DID document modification
- **DIDSuspended**: DID suspension
- **DIDRevoked**: DID revocation
- **DIDReactivated**: DID reactivation

## Error Codes

| Error | Description |
|-------|-------------|
| InvalidDIDFormat | Invalid DID format |
| DIDAlreadyExists | DID already exists for controller |
| DIDNotFound | DID not found |
| Unauthorized | Unauthorized operation |
| InvalidVerificationMethod | Invalid verification method |
| InvalidService | Invalid service definition |
| MaxVerificationMethodsExceeded | Too many verification methods |
| MaxServicesExceeded | Too many services |
| InvalidSignature | Invalid signature |
| DIDRevoked | DID is revoked |
| DIDSuspended | DID is suspended |
| InvalidController | Invalid controller address |
| InvalidPublicKey | Invalid public key |
| UnsupportedKeyType | Unsupported key type |
| RateLimitExceeded | Rate limit exceeded |
| AuditRequired | Audit logging required |

## Constants

- `MAX_VERIFICATION_METHODS`: 10
- `MAX_SERVICES`: 20
- `MAX_HISTORY_SIZE`: 1000
- `DID_PREFIX`: "did:stellar:"

## Security Considerations

1. **Authorization**: Only DID controllers can modify their DIDs
2. **Admin Controls**: Admin-only suspension/revocation for security
3. **Rate Limiting**: Protection against DoS attacks
4. **Audit Trail**: Complete operation logging
5. **Validation**: Comprehensive input validation

## Integration

### With Verifiable Credentials
The DID contract integrates with the Verifiable Credentials contract for:
- Credential issuer/subject identification
- Verification method validation
- Service endpoint discovery

### With Compliance
Integration with compliance system for:
- KYC/AML verification
- Risk assessment
- Regulatory compliance

## Usage Examples

### Creating a DID
```rust
let verification_method = VerificationMethod {
    id: "key-1".to_string(),
    type_: "Ed25519VerificationKey2018".to_string(),
    controller: "did:stellar:controller".to_string(),
    public_key: Bytes::from_slice(&public_key_bytes),
    created: timestamp,
};

let service = Service {
    id: "agent-registry".to_string(),
    type_: "AgentRegistry".to_string(),
    service_endpoint: "https://api.stellai.verse/agents".to_string(),
    created: timestamp,
};

let did = did_client.create_did(
    &controller_address,
    &vec![verification_method],
    &vec![service],
)?;
```

### Updating a DID
```rust
let new_version = did_client.update_did(
    &did,
    &controller_address,
    Some(&new_verification_methods),
    None,
)?;
```

### Checking DID Validity
```rust
let is_valid = did_client.is_valid_did(&did)?;
if is_valid {
    // Proceed with operations
}
```

## Testing

Run the test suite:
```bash
cargo test --package identity-did
```

## Deployment

1. Build the contract:
```bash
cargo build --release --package identity-did
```

2. Deploy to Stellar network:
```bash
stellar contract deploy ...
```

3. Initialize with admin:
```bash
stellar contract invoke \
  --id <contract_id> \
  --function initialize \
  --arg <admin_address>
```

## Standards Compliance

This contract implements:
- W3C DID Core Specification
- W3C DID Resolution
- Stellar DID Method (did:stellar)

## License

Copyright StellAIverse Team. All rights reserved.
