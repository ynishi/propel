use propel_sdk::state::PropelState;
use std::sync::Mutex;

/// Environment variable tests mutate process-global state, so we serialize them.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// SAFETY: Tests are serialized via ENV_LOCK, so no concurrent env mutation.
unsafe fn set_env(key: &str, val: &str) {
    // SAFETY: caller holds ENV_LOCK, ensuring single-threaded env access
    unsafe { std::env::set_var(key, val) };
}

/// SAFETY: Tests are serialized via ENV_LOCK, so no concurrent env mutation.
unsafe fn remove_env(key: &str) {
    // SAFETY: caller holds ENV_LOCK, ensuring single-threaded env access
    unsafe { std::env::remove_var(key) };
}

fn with_env<F, R>(vars: &[(&str, &str)], f: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = ENV_LOCK.lock().unwrap();

    for (k, v) in vars {
        // SAFETY: protected by ENV_LOCK
        unsafe { set_env(k, v) };
    }

    let result = f();

    for (k, _) in vars {
        // SAFETY: protected by ENV_LOCK
        unsafe { remove_env(k) };
    }

    result
}

fn clear_supabase_env() {
    // SAFETY: caller must hold ENV_LOCK
    unsafe {
        remove_env("SUPABASE_URL");
        remove_env("SUPABASE_ANON_KEY");
        remove_env("SUPABASE_JWT_SECRET");
    }
}

// ── Normal cases ──

#[test]
fn load_succeeds_with_all_env_vars() {
    with_env(
        &[
            ("SUPABASE_URL", "https://example.supabase.co"),
            ("SUPABASE_ANON_KEY", "anon-key-123"),
            ("SUPABASE_JWT_SECRET", "jwt-secret-456"),
        ],
        || {
            let state = PropelState::load().unwrap();
            assert_eq!(state.supabase_url, "https://example.supabase.co");
            assert_eq!(state.supabase_anon_key, "anon-key-123");
            assert_eq!(state.supabase_jwt_secret, "jwt-secret-456");
        },
    );
}

#[test]
fn load_preserves_exact_values() {
    with_env(
        &[
            ("SUPABASE_URL", "https://a.b.c"),
            ("SUPABASE_ANON_KEY", "key-with-special=chars/+"),
            ("SUPABASE_JWT_SECRET", "s3cr3t!@#$%"),
        ],
        || {
            let state = PropelState::load().unwrap();
            assert_eq!(state.supabase_url, "https://a.b.c");
            assert_eq!(state.supabase_anon_key, "key-with-special=chars/+");
            assert_eq!(state.supabase_jwt_secret, "s3cr3t!@#$%");
        },
    );
}

// ── Error cases ──

#[test]
fn load_fails_missing_supabase_url() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_supabase_env();
    // SAFETY: protected by ENV_LOCK
    unsafe {
        set_env("SUPABASE_ANON_KEY", "key");
        set_env("SUPABASE_JWT_SECRET", "secret");
    }

    let result = PropelState::load();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("SUPABASE_URL"),
        "error should name the missing var: {err}"
    );

    clear_supabase_env();
}

#[test]
fn load_fails_missing_anon_key() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_supabase_env();
    // SAFETY: protected by ENV_LOCK
    unsafe {
        set_env("SUPABASE_URL", "https://example.supabase.co");
        set_env("SUPABASE_JWT_SECRET", "secret");
    }

    let result = PropelState::load();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("SUPABASE_ANON_KEY"),
        "error should name the missing var: {err}"
    );

    clear_supabase_env();
}

#[test]
fn load_fails_missing_jwt_secret() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_supabase_env();
    // SAFETY: protected by ENV_LOCK
    unsafe {
        set_env("SUPABASE_URL", "https://example.supabase.co");
        set_env("SUPABASE_ANON_KEY", "key");
    }

    let result = PropelState::load();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("SUPABASE_JWT_SECRET"),
        "error should name the missing var: {err}"
    );

    clear_supabase_env();
}

#[test]
fn load_fails_all_missing() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_supabase_env();

    let result = PropelState::load();
    assert!(result.is_err());

    clear_supabase_env();
}
