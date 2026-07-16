//! Native backend used by the iced shell.
//!
//! The UI owns presentation only. Workspace state remains in
//! `~/.ducktape/registry.json`, and every blocking node/key operation is
//! serialized through one bounded [`NodeControl`] actor.

mod account_profile;
mod device_cache;
mod enroll;
mod gateway;
mod identity;
mod link;
mod node_control;
pub(crate) mod private_fs;
mod sandbox;
mod signing;
mod workspace_service;
mod workspaces;

use std::path::{Component, Path, PathBuf};

use node_control::NodeControl;

pub use crate::view_api::{
    LinkResponse, MemberKeyKind, decode_link_response, encode_link_response,
};
pub(crate) use account_profile::{LocalAccountProfile, LocalAccountProfilePatch, MAX_AVATAR_BYTES};
pub use device_cache::{CachedDeviceRow, CachedNetworkDevices, DeviceStanding};
#[allow(unused_imports)]
pub use enroll::{PhoneCandidate, PhoneEnrollmentStart};
#[allow(unused_imports)]
pub use identity::{
    IdentityCreated, IdentityMnemonic, IdentityPubkey, IdentityState, IdentityStatus,
    RecoveryPhrase,
};
#[allow(unused_imports)]
pub use link::{
    LinkAddress, LinkChallenge, LinkPending, LinkRelayStart, decode_link_challenge,
    encode_link_challenge,
};
pub use sandbox::{ProbeResult, SandboxChoice};
#[allow(unused_imports)]
pub use signing::{
    AddMemberRequest, BindRequest, ContentTarget, PossessionRequest, RemoveMemberRequest,
};
#[allow(unused_imports)]
pub use workspace_service::{
    WorkspaceActivation, WorkspaceInviteForms, WorkspaceJoinRequest, WorkspaceLogTail,
    WorkspaceNodeStatus, WorkspacePhaseReport,
};
#[allow(unused_imports)]
pub use workspaces::{Workspace, WorkspacePorts, WorkspaceSnapshot};

/// The smallest boot boundary the iced UI needs: fixed local paths plus the
/// process-control actor. Clone is cheap and safe to move into iced tasks.
#[derive(Debug, Clone)]
pub struct Backend {
    root: PathBuf,
    control: NodeControl,
}

impl Backend {
    /// Open the current user's `~/.ducktape` state.
    ///
    /// This is async so construction necessarily happens inside iced's Tokio
    /// executor; [`NodeControl`] starts its blocking receive loop there.
    pub async fn new() -> Result<Self, String> {
        refuse_elevated_process()?;
        let home = home_dir()?;
        Self::at_root(home.join(".ducktape")).await
    }

    /// Open an explicit registry root. Useful for isolated QA and tests.
    pub async fn at_root(root: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root.into();
        validate_root(&root)?;
        private_fs::ensure_private_dir(&root)?;
        Ok(Self {
            root,
            control: NodeControl::new()?,
        })
    }

    /// Read the registry once and return both the ordered workspace list and
    /// the active entry. Corrupt or unsafe registry data is backed up and
    /// recovered as the empty first-run state.
    pub async fn workspace_snapshot(&self) -> Result<WorkspaceSnapshot, String> {
        let root = self.root.clone();
        self.control
            .run(move || workspaces::snapshot_at(&root))
            .await
    }

    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>, String> {
        Ok(self.workspace_snapshot().await?.workspaces)
    }

    #[allow(dead_code)]
    pub async fn active_workspace(&self) -> Result<Option<Workspace>, String> {
        Ok(self.workspace_snapshot().await?.active)
    }

    /// The validated local state root. Native service adapters use this to
    /// locate workspace-owned materializations without rediscovering `$HOME`.
    pub(crate) fn state_root(&self) -> PathBuf {
        self.root.clone()
    }

    /// Stop the bounded blocking actor before iced tears down its Tokio runtime.
    pub(crate) fn shutdown(&self) {
        self.control.shutdown();
    }
}

fn refuse_elevated_process() -> Result<(), String> {
    #[cfg(unix)]
    // SAFETY: `geteuid` returns the process credential and retains no pointers.
    if unsafe { libc::geteuid() } == 0 {
        return Err("Ducktape Desktop must run as a regular user, never root".into());
    }
    #[cfg(windows)]
    // SAFETY: `IsUserAnAdmin` reads the current process token and retains no pointers.
    if unsafe { windows::Win32::UI::Shell::IsUserAnAdmin() }.as_bool() {
        return Err("Ducktape Desktop must run unelevated, never as administrator".into());
    }
    Ok(())
}

fn home_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let value = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let value = std::env::var_os("HOME");
    let home = value
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "could not determine the current user's home directory".to_string())?;
    validate_root(&home)?;
    Ok(home)
}

fn validate_root(root: &Path) -> Result<(), String> {
    if !root.is_absolute() {
        return Err(format!("backend root must be absolute: {}", root.display()));
    }
    if root
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(format!(
            "backend root must not contain relative path components: {}",
            root.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_root_is_absolute_and_normalized() {
        assert!(validate_root(Path::new("relative")).is_err());
        assert!(validate_root(Path::new("/tmp/../state")).is_err());
        assert!(validate_root(Path::new("/tmp/ducktape-state")).is_ok());
    }
}
