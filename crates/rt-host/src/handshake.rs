//! Per-method {major,minor} negotiation. Existing methods stay 1.0; policy.* is 1.1; write/git mutate is 1.2.

use std::collections::BTreeMap;

use rt_protocol::{
    host_method_version, method_version, ClientHello, MethodVersion, RejectedMethod, ServerHello,
    TRADABLE_METHODS,
};

pub const HOST_VERSION: &str = rt_protocol::CRATE_VERSION;
pub const HOST_METHOD_MAJOR: u32 = rt_protocol::HOST_METHOD_MAJOR;
pub const HOST_METHOD_MINOR: u32 = rt_protocol::HOST_METHOD_MINOR;

pub type HandshakeParams = ClientHello;
pub type HandshakeResult = ServerHello;

pub fn host_method_version_local() -> MethodVersion {
    host_method_version()
}

pub fn supported_methods() -> &'static [&'static str] {
    TRADABLE_METHODS
}

pub fn is_known_method(name: &str) -> bool {
    TRADABLE_METHODS.contains(&name)
}

pub fn host_knows(name: &str) -> bool {
    is_known_method(name)
}

pub fn host_methods() -> BTreeMap<String, MethodVersion> {
    TRADABLE_METHODS
        .iter()
        .filter_map(|m| method_version(m).map(|v| ((*m).to_string(), v)))
        .collect()
}

/// Accept when major is equal and client.minor ≤ host.minor.
/// Unknown name → rejected.reason = "unsupported".
/// Version rule fails → rejected.reason = "version_mismatch".
pub fn negotiate(
    client: &BTreeMap<String, MethodVersion>,
) -> (
    BTreeMap<String, MethodVersion>,
    BTreeMap<String, RejectedMethod>,
) {
    let host = host_methods();
    let mut accepted = BTreeMap::new();
    let mut rejected = BTreeMap::new();
    for (name, ver) in client {
        match host.get(name) {
            Some(h) if ver.major == h.major && ver.minor <= h.minor => {
                accepted.insert(name.clone(), *h);
            }
            Some(_) => {
                rejected.insert(
                    name.clone(),
                    RejectedMethod {
                        reason: "version_mismatch".into(),
                    },
                );
            }
            None => {
                rejected.insert(
                    name.clone(),
                    RejectedMethod {
                        reason: "unsupported".into(),
                    },
                );
            }
        }
    }
    (accepted, rejected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_reject_reason_is_version_mismatch() {
        let mut client = BTreeMap::new();
        client.insert("host.doctor".into(), MethodVersion { major: 2, minor: 0 });
        client.insert(
            "files.tree".into(),
            MethodVersion {
                major: 1,
                minor: 99,
            },
        );
        let (acc, rej) = negotiate(&client);
        assert!(!acc.contains_key("host.doctor"));
        assert!(!acc.contains_key("files.tree"));
        assert_eq!(rej["host.doctor"].reason, "version_mismatch");
        assert_eq!(rej["files.tree"].reason, "version_mismatch");
        let v = serde_json::to_value(&rej).unwrap();
        assert_eq!(v["host.doctor"]["reason"], "version_mismatch");
    }

    #[test]
    fn version_accept_equal_and_older_minor() {
        let mut client = BTreeMap::new();
        client.insert("task.create".into(), MethodVersion { major: 1, minor: 0 });
        client.insert("host.ping".into(), MethodVersion { major: 1, minor: 0 });
        client.insert("files.tree".into(), MethodVersion { major: 1, minor: 0 });
        client.insert("files.read".into(), MethodVersion { major: 1, minor: 0 });
        let (acc, rej) = negotiate(&client);
        assert!(acc.contains_key("task.create"));
        assert!(acc.contains_key("host.ping"));
        assert!(acc.contains_key("files.tree"));
        assert!(acc.contains_key("files.read"));
        assert!(rej.is_empty());
    }

    #[test]
    fn unknown_method_rejected_unsupported() {
        let mut client = BTreeMap::new();
        client.insert(
            "artifact.create".into(),
            MethodVersion { major: 1, minor: 0 },
        );
        let (acc, rej) = negotiate(&client);
        assert!(acc.is_empty());
        assert_eq!(rej["artifact.create"].reason, "unsupported");
    }

    #[test]
    fn worktree_and_git_methods_accepted_at_1_0() {
        let mut client = BTreeMap::new();
        for name in [
            "worktree.ensure",
            "worktree.get",
            "worktree.list",
            "git.status",
            "git.diff",
        ] {
            client.insert(name.into(), MethodVersion { major: 1, minor: 0 });
        }
        let (acc, rej) = negotiate(&client);
        assert!(rej.is_empty(), "{rej:?}");
        for name in [
            "worktree.ensure",
            "worktree.get",
            "worktree.list",
            "git.status",
            "git.diff",
        ] {
            assert_eq!(acc[name], MethodVersion { major: 1, minor: 0 });
        }
    }

    #[test]
    fn agent_cancel_accepted_at_1_0() {
        let mut client = BTreeMap::new();
        client.insert("agent.cancel".into(), MethodVersion { major: 1, minor: 0 });
        let (acc, rej) = negotiate(&client);
        assert!(rej.is_empty(), "{rej:?}");
        assert_eq!(acc["agent.cancel"], MethodVersion { major: 1, minor: 0 });
    }

    #[test]
    fn handshake_is_not_tradable() {
        let mut client = BTreeMap::new();
        client.insert("handshake".into(), MethodVersion { major: 1, minor: 0 });
        let (acc, rej) = negotiate(&client);
        assert!(acc.is_empty());
        assert_eq!(rej["handshake"].reason, "unsupported");
    }
    #[test]
    fn policy_methods_accepted_at_1_1() {
        let mut client = BTreeMap::new();
        client.insert("policy.get".into(), MethodVersion { major: 1, minor: 1 });
        client.insert("policy.set".into(), MethodVersion { major: 1, minor: 1 });
        client.insert(
            "approval.respond".into(),
            MethodVersion { major: 1, minor: 1 },
        );
        client.insert("leftover.foo".into(), MethodVersion { major: 1, minor: 0 });
        let (acc, rej) = negotiate(&client);
        assert_eq!(acc["policy.get"], MethodVersion { major: 1, minor: 1 });
        assert_eq!(acc["policy.set"], MethodVersion { major: 1, minor: 1 });
        assert_eq!(
            acc["approval.respond"],
            MethodVersion { major: 1, minor: 1 }
        );
        assert_eq!(rej["leftover.foo"].reason, "unsupported");

        let mut older = BTreeMap::new();
        older.insert("policy.get".into(), MethodVersion { major: 1, minor: 0 });
        let (acc, rej) = negotiate(&older);
        assert!(rej.is_empty(), "{rej:?}");
        assert_eq!(acc["policy.get"], MethodVersion { major: 1, minor: 1 });
    }

    #[test]
    fn helpers_expose_supported_methods() {
        let v = host_method_version_local();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(supported_methods().len(), TRADABLE_METHODS.len());
        assert!(is_known_method("host.ping"));
        assert!(host_knows("agent.cancel"));
        assert!(!host_knows("handshake"));
        let all = host_methods();
        assert_eq!(all.len(), TRADABLE_METHODS.len());
        assert_eq!(all["files.tree"], MethodVersion { major: 1, minor: 0 });
    }

    #[test]
    fn write_methods_accepted_at_1_2_tree_stays_1_0() {
        let names = [
            "files.write",
            "files.patch",
            "files.open",
            "git.stage",
            "git.unstage",
            "git.restore",
            "git.commit",
            "git.push",
        ];
        let mut client = BTreeMap::new();
        for name in names {
            client.insert(name.into(), MethodVersion { major: 1, minor: 2 });
        }
        client.insert("files.tree".into(), MethodVersion { major: 1, minor: 0 });
        let (acc, rej) = negotiate(&client);
        assert!(rej.is_empty(), "{rej:?}");
        for name in names {
            assert_eq!(acc[name], MethodVersion { major: 1, minor: 2 });
        }
        assert_eq!(acc["files.tree"], MethodVersion { major: 1, minor: 0 });
        assert_eq!(
            host_methods()["files.write"],
            MethodVersion { major: 1, minor: 2 }
        );
        assert_eq!(
            host_methods()["files.tree"],
            MethodVersion { major: 1, minor: 0 }
        );

        let mut older = BTreeMap::new();
        older.insert("files.write".into(), MethodVersion { major: 1, minor: 0 });
        let (acc, rej) = negotiate(&older);
        assert!(rej.is_empty(), "{rej:?}");
        assert_eq!(acc["files.write"], MethodVersion { major: 1, minor: 2 });
    }
}
