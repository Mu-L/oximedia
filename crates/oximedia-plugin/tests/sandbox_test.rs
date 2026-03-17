//! Integration tests for sandbox permission enforcement.
//!
//! Verifies that the sandbox correctly blocks operations the plugin does not
//! have permission to perform, and allows operations that are permitted.

use oximedia_plugin::{
    PermissionSet, PluginSandbox, SandboxConfig, SandboxContext, SandboxError, PERM_AUDIO,
    PERM_FILESYSTEM, PERM_GPU, PERM_MEMORY_LARGE, PERM_NETWORK, PERM_VIDEO,
};
use std::time::Duration;

// ── Helper ────────────────────────────────────────────────────────────────────

fn ctx_with_perms(flags: u32) -> SandboxContext {
    SandboxContext::new(SandboxConfig {
        permissions: PermissionSet::new().grant(flags),
        ..SandboxConfig::default()
    })
}

fn ctx_no_perms() -> SandboxContext {
    SandboxContext::new(SandboxConfig::default())
}

fn ctx_all_perms() -> SandboxContext {
    SandboxContext::new(SandboxConfig::permissive())
}

// ── Permission denial tests ────────────────────────────────────────────────

#[test]
fn test_network_denied_without_permission() {
    let ctx = ctx_no_perms();
    let err = ctx.check_permission(PERM_NETWORK).expect_err("should deny");
    assert!(
        matches!(err, SandboxError::PermissionDenied { requested, .. } if requested == PERM_NETWORK)
    );
}

#[test]
fn test_filesystem_denied_without_permission() {
    let ctx = ctx_no_perms();
    let err = ctx
        .check_permission(PERM_FILESYSTEM)
        .expect_err("should deny");
    assert!(matches!(
        err,
        SandboxError::PermissionDenied {
            requested,
            ..
        } if requested == PERM_FILESYSTEM
    ));
}

#[test]
fn test_gpu_denied_without_permission() {
    let ctx = ctx_no_perms();
    assert!(matches!(
        ctx.check_permission(PERM_GPU),
        Err(SandboxError::PermissionDenied { .. })
    ));
}

#[test]
fn test_audio_denied_without_permission() {
    let ctx = ctx_no_perms();
    assert!(matches!(
        ctx.check_permission(PERM_AUDIO),
        Err(SandboxError::PermissionDenied { .. })
    ));
}

#[test]
fn test_video_denied_without_permission() {
    let ctx = ctx_no_perms();
    assert!(matches!(
        ctx.check_permission(PERM_VIDEO),
        Err(SandboxError::PermissionDenied { .. })
    ));
}

#[test]
fn test_memory_large_denied_without_permission() {
    let ctx = ctx_no_perms();
    assert!(matches!(
        ctx.check_permission(PERM_MEMORY_LARGE),
        Err(SandboxError::PermissionDenied { .. })
    ));
}

// ── Permission grant tests ─────────────────────────────────────────────────

#[test]
fn test_network_allowed_with_permission() {
    let ctx = ctx_with_perms(PERM_NETWORK);
    assert!(ctx.check_permission(PERM_NETWORK).is_ok());
    // Other permissions are still denied.
    assert!(ctx.check_permission(PERM_FILESYSTEM).is_err());
}

#[test]
fn test_filesystem_allowed_with_permission() {
    let ctx = ctx_with_perms(PERM_FILESYSTEM);
    assert!(ctx.check_permission(PERM_FILESYSTEM).is_ok());
    assert!(ctx.check_permission(PERM_GPU).is_err());
}

#[test]
fn test_all_permissions_allowed_with_permissive_config() {
    let ctx = ctx_all_perms();
    assert!(ctx.check_permission(PERM_NETWORK).is_ok());
    assert!(ctx.check_permission(PERM_FILESYSTEM).is_ok());
    assert!(ctx.check_permission(PERM_GPU).is_ok());
    assert!(ctx.check_permission(PERM_AUDIO).is_ok());
    assert!(ctx.check_permission(PERM_VIDEO).is_ok());
    assert!(ctx.check_permission(PERM_MEMORY_LARGE).is_ok());
}

// ── Compound permission checks ─────────────────────────────────────────────

#[test]
fn test_compound_permission_all_bits_must_be_granted() {
    let ctx = ctx_with_perms(PERM_NETWORK); // only network
                                            // Requesting NETWORK | FILESYSTEM fails because FILESYSTEM is absent.
    assert!(ctx
        .check_permission(PERM_NETWORK | PERM_FILESYSTEM)
        .is_err());
}

#[test]
fn test_compound_permission_succeeds_when_all_granted() {
    let ctx = ctx_with_perms(PERM_NETWORK | PERM_FILESYSTEM);
    assert!(ctx.check_permission(PERM_NETWORK | PERM_FILESYSTEM).is_ok());
}

// ── Memory limit enforcement ──────────────────────────────────────────────

#[test]
fn test_memory_within_limit_allowed() {
    let ctx = SandboxContext::new(SandboxConfig {
        max_memory_mb: 1,
        ..SandboxConfig::default()
    });
    assert!(ctx.check_memory(512 * 1024).is_ok()); // 512 KiB < 1 MiB
}

#[test]
fn test_memory_exceeds_limit_denied() {
    let ctx = SandboxContext::new(SandboxConfig {
        max_memory_mb: 1,
        ..SandboxConfig::default()
    });
    let err = ctx.check_memory(2 * 1024 * 1024).expect_err("should deny");
    assert!(matches!(err, SandboxError::MemoryExceeded { .. }));
}

#[test]
fn test_memory_cumulative_exceeds_limit_denied() {
    let ctx = SandboxContext::new(SandboxConfig {
        max_memory_mb: 1,
        ..SandboxConfig::default()
    });
    ctx.check_memory(600 * 1024).expect("first");
    let err = ctx
        .check_memory(600 * 1024)
        .expect_err("second should exceed");
    assert!(matches!(err, SandboxError::MemoryExceeded { .. }));
    // First allocation should have been preserved.
    assert_eq!(ctx.used_memory_bytes(), 600 * 1024);
}

// ── Timeout enforcement ───────────────────────────────────────────────────

#[test]
fn test_timeout_not_yet_exceeded_allowed() {
    let ctx = SandboxContext::new(SandboxConfig {
        timeout_ms: 60_000,
        ..SandboxConfig::default()
    });
    assert!(ctx.check_timeout().is_ok());
}

#[test]
fn test_timeout_exceeded_denied() {
    let ctx = SandboxContext::new(SandboxConfig {
        timeout_ms: 0, // Immediately exceeded
        ..SandboxConfig::default()
    });
    std::thread::sleep(Duration::from_millis(1));
    let err = ctx.check_timeout().expect_err("should time out");
    assert!(matches!(err, SandboxError::Timeout { .. }));
}

// ── PluginSandbox::run enforcement ────────────────────────────────────────

#[test]
fn test_sandbox_run_enforces_permission_in_closure() {
    let sb = PluginSandbox::new(SandboxConfig::default()); // no perms
    let result = sb.run(|ctx| {
        ctx.check_permission(PERM_NETWORK)?;
        Ok(())
    });
    assert!(matches!(result, Err(SandboxError::PermissionDenied { .. })));
}

#[test]
fn test_sandbox_run_allows_permitted_operations() {
    let sb = PluginSandbox::new(SandboxConfig {
        permissions: PermissionSet::new().grant(PERM_FILESYSTEM),
        ..SandboxConfig::default()
    });
    let result = sb.run(|ctx| {
        ctx.check_permission(PERM_FILESYSTEM)?;
        ctx.check_memory(1024)?;
        Ok(42u32)
    });
    assert_eq!(result.expect("allowed"), 42);
}

#[test]
fn test_sandbox_run_enforces_memory_limit() {
    let sb = PluginSandbox::new(SandboxConfig {
        permissions: PermissionSet::with_all(),
        max_memory_mb: 1,
        ..SandboxConfig::default()
    });
    let result = sb.run(|ctx| {
        ctx.check_memory(2 * 1024 * 1024)?; // 2 MiB > limit
        Ok(())
    });
    assert!(matches!(result, Err(SandboxError::MemoryExceeded { .. })));
}

// ── Error display messages ────────────────────────────────────────────────

#[test]
fn test_permission_denied_error_displays_hex_flags() {
    let err = SandboxError::PermissionDenied {
        requested: PERM_NETWORK,
        available: 0x00,
    };
    let msg = err.to_string();
    assert!(msg.contains("permission denied"));
    assert!(msg.contains("01")); // hex of PERM_NETWORK
}

#[test]
fn test_memory_exceeded_error_displays_bytes() {
    let err = SandboxError::MemoryExceeded {
        used: 2 * 1024 * 1024,
        limit: 1 * 1024 * 1024,
    };
    let msg = err.to_string();
    assert!(msg.contains("memory limit exceeded"));
}

#[test]
fn test_timeout_error_displays_elapsed_ms() {
    let err = SandboxError::Timeout { elapsed_ms: 7_500 };
    let msg = err.to_string();
    assert!(msg.contains("timeout"));
    assert!(msg.contains("7500"));
}
