//! Collaborative permission management with hierarchical access levels and
//! time-bounded access grants.

#![allow(dead_code)]

/// A permission level that can be granted to a user on a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Permission {
    /// Can view the resource.
    View,
    /// Can leave comments on the resource.
    Comment,
    /// Can make edits to the resource.
    Edit,
    /// Can manage collaborators and settings.
    Manage,
    /// Full ownership of the resource.
    Own,
}

impl Permission {
    /// Return `true` if this permission level allows write operations.
    #[must_use]
    pub fn allows_write(&self) -> bool {
        matches!(self, Self::Edit | Self::Manage | Self::Own)
    }

    /// Numeric level of this permission (higher = more privileged).
    #[must_use]
    pub fn level(&self) -> u8 {
        match self {
            Self::View => 1,
            Self::Comment => 2,
            Self::Edit => 3,
            Self::Manage => 4,
            Self::Own => 5,
        }
    }

    /// Return `true` if this permission level subsumes `other`.
    ///
    /// `Own` implies all others; `View` implies only itself.
    #[must_use]
    pub fn implies(&self, other: &Permission) -> bool {
        self.level() >= other.level()
    }
}

/// A single access grant associating a user with a permission on a resource.
#[derive(Debug, Clone)]
pub struct AccessGrant {
    /// Identifier of the user receiving the grant.
    pub user_id: String,
    /// Identifier of the resource the grant applies to.
    pub resource_id: String,
    /// Permission level granted.
    pub permission: Permission,
    /// Wall-clock time the grant was issued, in milliseconds since the Unix epoch.
    pub granted_ms: u64,
    /// Optional expiry time in milliseconds; `None` means the grant never expires.
    pub expires_ms: Option<u64>,
}

impl AccessGrant {
    /// Return `true` when this grant has an expiry and `now_ms` is past it.
    #[must_use]
    pub fn is_expired(&self, now_ms: u64) -> bool {
        self.expires_ms.map_or(false, |exp| now_ms > exp)
    }

    /// Return `true` when this grant exists and is not yet expired.
    #[must_use]
    pub fn is_valid(&self, now_ms: u64) -> bool {
        !self.is_expired(now_ms)
    }
}

/// A collection of `AccessGrant`s with query and mutation helpers.
#[derive(Debug, Default)]
pub struct AccessControl {
    /// All grants stored in insertion order.
    pub grants: Vec<AccessGrant>,
}

impl AccessControl {
    /// Create an empty `AccessControl`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new grant for `user_id` on `resource_id` with the given `permission`.
    pub fn grant(
        &mut self,
        user_id: impl Into<String>,
        resource_id: impl Into<String>,
        perm: Permission,
        now_ms: u64,
        expires_ms: Option<u64>,
    ) {
        self.grants.push(AccessGrant {
            user_id: user_id.into(),
            resource_id: resource_id.into(),
            permission: perm,
            granted_ms: now_ms,
            expires_ms,
        });
    }

    /// Remove all grants for `user_id` on `resource_id`.
    ///
    /// Returns `true` if at least one grant was removed.
    pub fn revoke(&mut self, user_id: &str, resource_id: &str) -> bool {
        let before = self.grants.len();
        self.grants
            .retain(|g| !(g.user_id == user_id && g.resource_id == resource_id));
        self.grants.len() < before
    }

    /// Return `true` if `user_id` has at least `needed` access on `resource_id`
    /// via any currently valid grant.
    #[must_use]
    pub fn check(
        &self,
        user_id: &str,
        resource_id: &str,
        needed: &Permission,
        now_ms: u64,
    ) -> bool {
        self.grants.iter().any(|g| {
            g.user_id == user_id
                && g.resource_id == resource_id
                && g.is_valid(now_ms)
                && g.permission.implies(needed)
        })
    }

    /// Return the resource ids of all resources `user_id` has any valid grant on.
    #[must_use]
    pub fn user_resources(&self, user_id: &str, now_ms: u64) -> Vec<&str> {
        let mut resources: Vec<&str> = self
            .grants
            .iter()
            .filter(|g| g.user_id == user_id && g.is_valid(now_ms))
            .map(|g| g.resource_id.as_str())
            .collect();
        resources.sort_unstable();
        resources.dedup();
        resources
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Permission ----

    #[test]
    fn test_permission_levels_ordered() {
        assert!(Permission::Own.level() > Permission::Manage.level());
        assert!(Permission::Manage.level() > Permission::Edit.level());
        assert!(Permission::Edit.level() > Permission::Comment.level());
        assert!(Permission::Comment.level() > Permission::View.level());
    }

    #[test]
    fn test_allows_write_true_for_edit_manage_own() {
        assert!(Permission::Edit.allows_write());
        assert!(Permission::Manage.allows_write());
        assert!(Permission::Own.allows_write());
    }

    #[test]
    fn test_allows_write_false_for_view_comment() {
        assert!(!Permission::View.allows_write());
        assert!(!Permission::Comment.allows_write());
    }

    #[test]
    fn test_own_implies_all() {
        for p in [
            Permission::View,
            Permission::Comment,
            Permission::Edit,
            Permission::Manage,
            Permission::Own,
        ] {
            assert!(Permission::Own.implies(&p));
        }
    }

    #[test]
    fn test_view_implies_only_itself() {
        assert!(Permission::View.implies(&Permission::View));
        assert!(!Permission::View.implies(&Permission::Comment));
    }

    #[test]
    fn test_implies_is_transitive() {
        // Edit implies Comment, Comment implies View → Edit implies View
        assert!(Permission::Edit.implies(&Permission::Comment));
        assert!(Permission::Edit.implies(&Permission::View));
    }

    // ---- AccessGrant ----

    #[test]
    fn test_grant_not_expired_when_no_expiry() {
        let g = AccessGrant {
            user_id: "u1".into(),
            resource_id: "r1".into(),
            permission: Permission::View,
            granted_ms: 0,
            expires_ms: None,
        };
        assert!(!g.is_expired(u64::MAX));
        assert!(g.is_valid(u64::MAX));
    }

    #[test]
    fn test_grant_expired_past_deadline() {
        let g = AccessGrant {
            user_id: "u1".into(),
            resource_id: "r1".into(),
            permission: Permission::View,
            granted_ms: 0,
            expires_ms: Some(1000),
        };
        assert!(g.is_expired(2000));
        assert!(!g.is_valid(2000));
    }

    #[test]
    fn test_grant_not_expired_before_deadline() {
        let g = AccessGrant {
            user_id: "u1".into(),
            resource_id: "r1".into(),
            permission: Permission::Edit,
            granted_ms: 0,
            expires_ms: Some(5000),
        };
        assert!(!g.is_expired(4999));
        assert!(g.is_valid(4999));
    }

    // ---- AccessControl ----

    #[test]
    fn test_access_control_check_granted() {
        let mut ac = AccessControl::new();
        ac.grant("alice", "res-1", Permission::Edit, 0, None);
        assert!(ac.check("alice", "res-1", &Permission::Edit, 0));
        assert!(ac.check("alice", "res-1", &Permission::View, 0)); // Edit implies View
    }

    #[test]
    fn test_access_control_check_not_enough_permission() {
        let mut ac = AccessControl::new();
        ac.grant("alice", "res-1", Permission::View, 0, None);
        assert!(!ac.check("alice", "res-1", &Permission::Edit, 0));
    }

    #[test]
    fn test_access_control_check_expired_grant() {
        let mut ac = AccessControl::new();
        ac.grant("alice", "res-1", Permission::Own, 0, Some(500));
        assert!(!ac.check("alice", "res-1", &Permission::View, 1000)); // expired
    }

    #[test]
    fn test_access_control_revoke_returns_true() {
        let mut ac = AccessControl::new();
        ac.grant("alice", "res-1", Permission::Edit, 0, None);
        assert!(ac.revoke("alice", "res-1"));
        assert!(!ac.check("alice", "res-1", &Permission::Edit, 0));
    }

    #[test]
    fn test_access_control_revoke_missing_returns_false() {
        let mut ac = AccessControl::new();
        assert!(!ac.revoke("nobody", "res-x"));
    }

    #[test]
    fn test_access_control_user_resources() {
        let mut ac = AccessControl::new();
        ac.grant("alice", "res-1", Permission::View, 0, None);
        ac.grant("alice", "res-2", Permission::Edit, 0, None);
        ac.grant("bob", "res-1", Permission::View, 0, None);
        let res = ac.user_resources("alice", 0);
        assert_eq!(res.len(), 2);
        assert!(res.contains(&"res-1"));
        assert!(res.contains(&"res-2"));
    }

    #[test]
    fn test_access_control_user_resources_excludes_expired() {
        let mut ac = AccessControl::new();
        ac.grant("alice", "res-1", Permission::View, 0, Some(100));
        let res = ac.user_resources("alice", 500);
        assert!(res.is_empty());
    }
}
