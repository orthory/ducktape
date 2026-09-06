//! The join ceremony: a pasted invite blob in, a materialized workspace out.
//!
//! Every other node operation has a running node to ask. This one does not —
//! joining is what BRINGS a workspace into existence, so there is no daemon
//! yet, no HTTP surface, and nothing to route the request to. That is why this
//! is a library and not an endpoint: the two programs that join a network (the
//! CLI and the desktop app) both have to be able to do it themselves.
//!
//! What lands on disk, in order, and why the order is the order: the identity
//! and the descriptor guard run FIRST, so a wrong-network paste or an
//! unreadable existing `node.toml` aborts before anything is written. Then the
//! descriptor, the plumbing, the invite token, the offered fronts and the
//! tunnel bootstrap — the set `node run` needs to race every first-contact path
//! the invite offers.

use std::path::{Path, PathBuf};

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519};

use crate::{
    Invite, Plumbing, Reach, ReachHint, SandboxToml, decode_invite, default_workspace_dir,
    guard_join_descriptor, hex_bytes, list_workspaces_in, load_or_generate_identity,
    merged_plumbing, save_invite_fronts, save_invite_token, save_invite_wireguard,
    validate_chain_id_shape, workspaces_root, write_node_toml,
};

/// Plumbing the joiner wants instead of the defaults. Every field is an
/// override: `None` keeps an existing `node.toml`'s value, else the working
/// default. The CLI fills these from its flags; a caller with no opinion passes
/// [`PlumbingOverrides::default`].
///
/// Owned here rather than taken as the CLI's `clap` struct, because a library
/// that reads a workspace has no business knowing what an argv looks like.
#[derive(Clone, Debug, Default)]
pub struct PlumbingOverrides {
    pub listen: Option<String>,
    pub advertised: Option<String>,
    pub http: Option<String>,
    pub gateway: Option<String>,
    pub rpc: Option<String>,
    pub primary_coordinator: Option<String>,
    pub wireguard_listen: Option<String>,
    pub wireguard_advertised: Option<String>,
    pub invite_listen: Option<String>,
}

/// What a join produced. Enough for either caller to say what happened without
/// re-reading the directory it just wrote.
#[derive(Clone, Debug)]
pub struct JoinedWorkspace {
    /// the network this workspace belongs to.
    pub chain_id: String,
    /// where it materialized.
    pub dir: PathBuf,
    /// the identity that will redeem the invite, hex.
    pub identity: String,
    /// the key file was MINTED by this join rather than reused. A re-join for
    /// the same chain reuses the identity already in that directory, which is
    /// what makes a re-paste with a fresh invite safe.
    pub generated: bool,
    /// this identity is already in the genesis validator set, so the node comes
    /// up as a member rather than redeeming its way in.
    pub is_member: bool,
    /// the compute runtime detection found on this host, when the workspace was
    /// fresh enough to get a live `[sandbox]` table. `None` means the node will
    /// boot consensus-only until someone uncomments the table.
    pub compute_runtime: Option<String>,
}

/// Materialize the workspace an invite admits this device to.
///
/// `dir` is the explicit destination; `None` puts it in the registry under the
/// invite's chain id, so the joined node is `-n <chain-id>`-addressable and a
/// re-join for the same chain lands in the same directory and reuses its
/// identity.
pub fn join_workspace(
    blob: &str,
    dir: Option<PathBuf>,
    overrides: &PlumbingOverrides,
) -> Result<JoinedWorkspace, String> {
    let invite = decode_invite(blob)?;
    let mut descriptor = invite.descriptor.clone();
    // the chain id came straight out of an untrusted invite: constrain its
    // shape to what `node init` actually mints before it is trusted to
    // address a registry directory or a `-n <chain-id>` selector.
    validate_chain_id_shape(&descriptor.chain_id)?;
    let dir = match dir {
        Some(dir) => dir,
        None => {
            guard_no_chain_id_collision(&workspaces_root()?, &descriptor.chain_id)?;
            default_workspace_dir(&descriptor.chain_id)?
        }
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;

    // Mint (or reuse) this workspace dir's identity. Every invite is BEARER, so
    // there is no target to match: any freshly minted key may redeem. The
    // redeeming key is bound by the join proof and the token is single-use, so
    // a paste simply admits whoever runs it.
    let (key, generated) = load_or_generate_identity(&dir.join("identity.key"))?;
    let identity = hex_bytes(key.public_key().as_ref());
    // the issuer `decode_invite` already verified the envelope against — the
    // one field of a pasted blob an attacker cannot choose freely, and what
    // separates a real admit refresh from a descriptor that merely copied our
    // validator list.
    guard_join_descriptor(&dir, &descriptor, &invite.token.issuer)?;

    // Computed BEFORE anything lands on disk, so a corrupt existing node.toml
    // aborts the join instead of leaving a half-written directory.
    let fresh_workspace = !dir.join("node.toml").exists();
    let mut plumbing = merged_plumbing(&dir, overrides)?;
    // A FRESH joining workspace gets the same compute detection as `init`: the
    // platform runtime on PATH ⇒ a live `[sandbox]` table (announce stays off),
    // so agent runs and the terminal plane work without a config edit. A
    // re-join over an existing node.toml keeps the operator's choice.
    let detected = fresh_workspace.then(detect_platform_sandbox).flatten();
    let compute_runtime = detected.map(|(table, _found)| {
        let runtime = table.runtime.clone();
        plumbing.sandbox = Some(table);
        runtime
    });

    fold_overlay_reach_hints(&invite, &mut descriptor)?;

    descriptor.save(&dir.join("network.toml"))?;
    write_node_toml(&dir, &plumbing)?;
    // the capability the joining node redeems automatically; a re-join with a
    // fresh invite replaces a stale/spent one.
    save_invite_token(&dir, &invite.token)?;
    // the offered fronts, kept beside the token so `run_node` can race the
    // whole union of first-contact paths. Empty clears any stale set.
    save_invite_fronts(&dir, &invite.fronts)?;
    // the tunnel bootstrap the joining node dials BEFORE any p2p (always
    // present); kept beside the token so `run_node` brings the interface up
    // first.
    save_invite_wireguard(&dir, &invite.token.issuer, &invite.wireguard)?;
    // mint the WireGuard identity NOW so the run's plane and intro announcer
    // read one settled key file instead of racing to create it.
    reachability::WireGuardKeypair::load_or_generate(&dir.join("wireguard.key"))
        .map_err(|e| format!("wireguard key: {e}"))?;

    Ok(JoinedWorkspace {
        is_member: descriptor.validators.contains(&identity),
        chain_id: descriptor.chain_id,
        dir,
        identity,
        generated,
        compute_runtime,
    })
}

/// refuse a chain id that would make `-n <chain-id>` prefix lookup ambiguous
/// against an ALREADY-registered network: `find_workspace_config_in` answers
/// an exact match before it ever looks at prefixes, so a chain id that equals
/// a habitual short selector (or is a prefix/extension of an existing id)
/// would silently steal that selector out from under an operator's existing
/// network. The exact-same-id case is not a collision — it is the same
/// network's own directory, and a re-join over it is legitimate.
fn guard_no_chain_id_collision(root: &Path, chain_id: &str) -> Result<(), String> {
    for (existing, _) in list_workspaces_in(root)? {
        if existing == chain_id {
            continue;
        }
        let shadows = existing.starts_with(chain_id) || chain_id.starts_with(&existing);
        if shadows {
            return Err(format!(
                "chain id {chain_id:?} collides with the already-registered network \
                 {existing:?} under `-n <chain-id>` prefix matching — refusing to create a \
                 workspace whose id would shadow it (or be shadowed by it); pass --dir to join \
                 outside the registry"
            ));
        }
    }
    Ok(())
}

/// A WireGuard or Coordinated invite makes the reachability plane the dial
/// path: fold the inviter's — and every offered front's — overlay ULA into this
/// joiner's reach hints, so the mesh can dial them the moment a tunnel is up.
fn fold_overlay_reach_hints(
    invite: &Invite,
    descriptor: &mut crate::NetworkDescriptor,
) -> Result<(), String> {
    if !crate::invite_requires_reachability_defaults(invite) {
        return Ok(());
    }
    let namespace = descriptor.genesis_namespace();
    let issuer_identity = wireguard::ValidatorIdentity::try_from(invite.token.issuer.as_ref())
        .map_err(|e| format!("inviter identity: {e:?}"))?;
    let inviter_ula = wireguard::ula_v6_member_addr(&namespace, issuer_identity);
    descriptor.add_reach_route(&ReachHint {
        expected_key: invite.token.issuer.clone(),
        reach: Reach::Direct(format!("[{inviter_ula}]:{}", invite.wireguard.mesh_port)),
    });
    for front in &invite.fronts {
        // a front whose key does not decode is skipped, not fatal: the invite
        // still carries the inviter's own paths, and refusing the whole join
        // over one malformed entry would strand a joiner with no way forward.
        let Ok(member) = ed25519::PublicKey::decode(&front.member_key[..]) else {
            continue;
        };
        let Ok(identity) = wireguard::ValidatorIdentity::try_from(&front.member_key[..]) else {
            continue;
        };
        let ula = wireguard::ula_v6_member_addr(&namespace, identity);
        descriptor.add_reach_route(&ReachHint {
            expected_key: member,
            reach: Reach::Direct(format!("[{ula}]:{}", front.mesh_port)),
        });
    }
    Ok(())
}

/// The `[sandbox]` table this platform would write, and the backend that same
/// table resolves to.
///
/// Both come from ONE call so a host can never be probed for one thing and
/// configured for another. One adapter per OS: Firecracker on Linux, the vz
/// shim on macOS. The written `0`s are "probe the host at boot", so the table
/// carries no machine's CPU/RAM into a config that travels.
///
/// The images need not exist yet — `init`/`join` run
/// before `ops/build-guest-rootfs.sh` on a fresh box, and the loud error
/// belongs to the boot probe, where an operator who uncommented the table is
/// standing. [`detect_platform_sandbox`] therefore probes the ADAPTER only.
pub fn platform_sandbox() -> Result<(SandboxToml, sandbox_host::SandboxBackend), String> {
    let vmm = sandbox_host::Vmm::platform_default();
    let guest = crate::default_guest_dir()?;
    let (kernel, rootfs) = (guest.join("vmlinux"), guest.join("rootfs.ext4"));
    let backend = sandbox_host::SandboxBackend::MicroVm {
        vmm,
        kernel: kernel.clone(),
        rootfs: rootfs.clone(),
        executors: crate::executor_dir()?,
    };
    let table = SandboxToml {
        runtime: vmm.config_token().into(),
        kernel,
        rootfs,
        // `0` is "probe the host at boot", not "no cores" — a written table must
        // not pin this box's CPU/RAM into a config that travels.
        cores: 0,
        mem_gb: 0,
    };
    Ok((table, backend))
}

/// Fresh-workspace compute detection: the platform adapter's runtime binary on
/// PATH ⇒ a live `[sandbox]` table and the binary the probe actually found;
/// absent ⇒ `None` (the commented example).
///
/// [`sandbox_host::SandboxBackend::probe_adapter`], NOT the full boot probe:
/// the question here is whether this HOST can isolate a run, and the images
/// are built after `init` on a fresh box. Asking the full probe made a machine
/// that can run providers write the commented table, and the operator then met
/// `no [sandbox] table in node.toml` instead of `build it with
/// ops/build-guest-rootfs.sh`.
///
/// The path comes back because WHICH `firecracker` answered is the fact an
/// operator with several on `PATH` needs, and only the probe knows it.
pub fn detect_platform_sandbox() -> Option<(SandboxToml, PathBuf)> {
    let (table, backend) = platform_sandbox().ok()?;
    backend.probe_adapter().ok().map(|found| (table, found))
}

/// The plumbing a caller with no overrides gets — exposed because both callers
/// want to show it before writing it.
pub fn default_plumbing(dir: &Path) -> Result<Plumbing, String> {
    merged_plumbing(dir, &PlumbingOverrides::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table `join`/`init` would write and the backend the probe would test
    /// must name ONE runtime and ONE pair of images — a drift here surfaces as
    /// a boot error on a machine whose images are exactly where init said.
    #[test]
    fn the_written_table_and_the_probed_backend_name_one_runtime() {
        let (table, backend) = platform_sandbox().expect("platform sandbox");
        // whether `SandboxBackend::Bare` exists here is decided by feature
        // unification — a workspace build turns sandbox-host's `testkit` on
        // through some other crate's dev-dependency, `-p workspace-config`
        // does not. so this destructure is refutable in one build and
        // irrefutable in the other: keep the guard, which names what the
        // comparison needs and FAILS loudly rather than silently skipping it.
        #[allow(irrefutable_let_patterns)]
        let sandbox_host::SandboxBackend::MicroVm {
            vmm,
            kernel,
            rootfs,
            executors,
        } = &backend
        else {
            panic!("test expects the MicroVm backend, got {backend:?}")
        };
        assert_eq!(executors, &crate::executor_dir().expect("executor dir"));
        assert_eq!(table.runtime, vmm.config_token());
        assert_eq!((&table.kernel, &table.rootfs), (kernel, rootfs));

        let guest = crate::default_guest_dir().expect("guest dir");
        assert_eq!(table.kernel, guest.join("vmlinux"));
        assert_eq!(table.rootfs, guest.join("rootfs.ext4"));
        assert_eq!((table.cores, table.mem_gb), (0, 0));
    }

    /// a chain id that is a strict prefix of an already-registered network's
    /// id (or has one as its own prefix) is refused: `find_workspace_config_in`
    /// answers an EXACT match before it ever considers a prefix, so letting
    /// this land would silently steal an operator's habitual short `-n`
    /// selector for the existing network out from under them.
    #[test]
    fn a_chain_id_that_would_shadow_an_existing_workspace_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let existing = root.path().join("dognet#a1b2c3d4");
        std::fs::create_dir_all(&existing).unwrap();
        crate::NetworkDescriptor {
            chain_id: "dognet#a1b2c3d4".into(),
            validators: vec![],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
            block_time_ms: crate::DEFAULT_BLOCK_TIME_MS,
            genesis: String::new(),
            modules: Vec::new(),
        }
        .save(&existing.join("network.toml"))
        .unwrap();

        // a strict prefix of the existing id.
        let err = guard_no_chain_id_collision(root.path(), "dognet").unwrap_err();
        assert!(err.contains("dognet#a1b2c3d4"), "{err}");

        // the existing id is itself a strict prefix of the incoming one.
        let err = guard_no_chain_id_collision(root.path(), "dognet#a1b2c3d4ff").unwrap_err();
        assert!(err.contains("dognet#a1b2c3d4"), "{err}");

        // the SAME id is not a collision — it is a re-join of the same network.
        guard_no_chain_id_collision(root.path(), "dognet#a1b2c3d4")
            .expect("re-joining the same network is not a collision");

        // an unrelated id is fine.
        guard_no_chain_id_collision(root.path(), "cathouse#deadbeef")
            .expect("an unrelated chain id is not a collision");
    }

    /// A blob that is not an invite must fail BEFORE anything is written: the
    /// destination directory is derived from the invite's own chain id, so a
    /// join that got that far would have created a directory named after
    /// garbage.
    #[test]
    fn a_blob_that_is_not_an_invite_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("workspace");
        let overrides = PlumbingOverrides::default();
        assert!(join_workspace("not-an-invite", Some(target.clone()), &overrides).is_err());
        assert!(!target.exists(), "a refused join left a directory behind");
    }

    /// THE beat test: a joiner with NO flags at all — the desktop app's only
    /// shape (`join_workspace(&blob, None, &Default::default())`) — comes up on
    /// the founder's cadence, not the compiled default. The beat is a genesis
    /// fact carried by the invite, so there is nothing left in `node.toml` for
    /// a member to disagree about.
    #[test]
    fn a_joiner_inherits_the_founders_beat_with_no_flag() {
        const FOUNDING_BEAT: u64 = 250;
        assert_ne!(FOUNDING_BEAT, crate::DEFAULT_BLOCK_TIME_MS);

        let issuer = ed25519::PrivateKey::from_seed(31);
        let founder = issuer.public_key();
        let mut descriptor = crate::NetworkDescriptor {
            chain_id: "beat#a1b2c3d4".into(),
            validators: vec![hex_bytes(founder.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
            block_time_ms: FOUNDING_BEAT,
            genesis: "ab".repeat(32),
            modules: Vec::new(),
        };
        descriptor.add_bootstrap(&founder, "127.0.0.1:52200");
        let token =
            crate::mint_invite_token(&issuer, descriptor.genesis_namespace().as_bytes(), u64::MAX);
        let wireguard = crate::InviteWireGuard {
            public_key: [0u8; 32],
            endpoint: None,
            intro: None,
            mesh_port: 52200,
        };
        let blob = crate::encode_invite(&descriptor, &token, &wireguard, &[], &issuer)
            .expect("encode the invite");

        let root = tempfile::tempdir().unwrap();
        let joined = join_workspace(
            &blob,
            Some(root.path().join("joined")),
            &PlumbingOverrides::default(),
        )
        .expect("join");

        let landed = crate::NetworkDescriptor::load(&joined.dir.join("network.toml"))
            .expect("the joined descriptor");
        assert_eq!(landed.block_time_ms, FOUNDING_BEAT);
        let node_toml = std::fs::read_to_string(joined.dir.join("node.toml")).expect("node.toml");
        assert!(
            !node_toml.contains("block_time_ms"),
            "the beat is descriptor-only; node.toml must not restate it:\n{node_toml}"
        );
    }
}
