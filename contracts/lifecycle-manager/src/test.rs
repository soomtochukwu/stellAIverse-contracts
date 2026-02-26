#[test]
fn test_extend_ttl_active() {
    let env = Env::default();
    let key = Symbol::new(&env, "agent_1");
    env.storage().persistent().set(&key, &1u32);

    LifecycleManager::extend_ttl(env.clone(), key.clone(), DataLifecycle::Active);
    let ttl = env.storage().persistent().ttl(&key).unwrap();
    assert_eq!(ttl, ACTIVE_TTL);
}

#[test]
fn test_cleanup_expired() {
    let env = Env::default();
    let key = Symbol::new(&env, "evolution_1");
    env.storage().persistent().set(&key, &1u32);
    env.storage().persistent().remove(&key); // simulate expiration

    LifecycleManager::cleanup_expired_evolution(env.clone(), key.clone());
    assert!(!env.storage().persistent().has(&key));
}

#[test]
fn test_archive_listing() {
    let env = Env::default();
    let active_key = Symbol::new(&env, "listing_1");
    let archived_key = Symbol::new(&env, "listing_1_archived");
    env.storage().persistent().set(&active_key, &Bytes::from_array(&env, &[1,2,3]));

    LifecycleManager::archive_listing(env.clone(), active_key.clone(), archived_key.clone());
    assert!(!env.storage().persistent().has(&active_key));
    assert!(env.storage().persistent().has(&archived_key));
}
