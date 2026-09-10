//! Where a module-owned view comes from: the deployed artifact of the module
//! it belongs to. The registry names the module's ACTIVE code hash
//! (`ModulesQuery::ModuleStatus`), the artifact under that hash is fetched
//! and verified (`view_artifact`), and its view — component bytes plus the
//! assets shipped beside them — is handed over as one unit. A pending swap is
//! never read: the view drawn is the view of the code that runs.
//!
//! Every reading here is strict. A registry reply of another shape, a module
//! the registry does not list or lists twice, a hash that is not 32 bytes,
//! a fetch that fails or a body that does not hash to what was asked for is
//! an [`Error`], never a fallback to a desktop resource. The only quiet
//! outcomes are the two the network itself asserts: a module admitted but
//! not yet activated ([`ViewSource::NotActivated`]) and a verified
//! deployment that ships no view ([`ViewSource::Missing`]).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ducktape_rpc::Client;
use serde::Deserialize;

use super::view_artifact;

/// The modules whose tab is drawn by the view in their own artifact, asked
/// of the connected node at connect and again at every block that moves
/// the deployment.
pub const MODULE_OWNED: [&str; 5] = ["governance", "files", "pages", "chat", "forge"];

/// The desktop's own views, staged beside the binary and asked for at boot.
/// With [`MODULE_OWNED`], every view the app draws: a tab's draw never
/// starts a load, so a view named in neither would never be there.
pub const DESKTOP_OWNED: [&str; 5] = ["members", "agents", "node", "explorer", "settings"];

pub fn module_owned(module: &str) -> bool {
    MODULE_OWNED.contains(&module)
}

/// The assets a deployment ships beside its view, by canonical relative
/// path. Shared between the guest and the host surfaces that paint them,
/// and swapped with the guest as one unit.
pub type Assets = BTreeMap<String, Vec<u8>>;

#[derive(Debug)]
pub enum Error {
    /// The registry could not be read as `module_status`, or does not name
    /// this module exactly once with a 32-byte active hash.
    Status(String),
    Artifact(view_artifact::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Status(error) => write!(formatter, "module status: {error}"),
            Self::Artifact(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, PartialEq)]
pub enum ViewSource {
    /// Admitted, but no activation has reached its boundary yet.
    NotActivated,
    /// The active deployment verified, and it ships no view.
    Missing { hash: [u8; 32] },
    Ready {
        hash: [u8; 32],
        component: Vec<u8>,
        assets: Arc<Assets>,
    },
}

// `ModulesReply::ModuleStatus`, as `/v1/query` serializes it — mirrored
// field for field from crates/modules/system/modules/src/interface.rs
// (`ModuleCode`, `ScheduledSwap`, `Activation`) and refused on any drift:
// a field this reader does not know is a registry it does not understand.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusReply {
    module_status: ModuleStatus,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleStatus {
    modules: Vec<ModuleCode>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleCode {
    module_id: String,
    active_code_hash: Vec<u8>,
    #[allow(
        dead_code,
        reason = "read for shape only: a pending swap is never loaded"
    )]
    pending: Option<ScheduledSwap>,
    #[allow(dead_code, reason = "read for shape only")]
    history: Vec<Activation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code, reason = "read for shape only")]
struct ScheduledSwap {
    name: String,
    activation_height: u64,
    code_hash: Vec<u8>,
    readiness: Vec<Vec<u8>>,
    ready_at: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code, reason = "read for shape only")]
struct Activation {
    height: u64,
    code_hash: Vec<u8>,
}

/// Every registered module's active code hash — `None` for an admission
/// that has not reached its boundary — in one registry read.
pub async fn active_hashes(client: &Client) -> Result<BTreeMap<String, Option<[u8; 32]>>, Error> {
    let reply: StatusReply = client
        .query("modules", &serde_json::json!("module_status"))
        .await
        .map_err(|error| Error::Status(error.to_string()))?;
    let mut hashes = BTreeMap::new();
    for entry in reply.module_status.modules {
        let module = entry.module_id;
        let hash = if entry.active_code_hash.is_empty() {
            None
        } else {
            Some(entry.active_code_hash.as_slice().try_into().map_err(|_| {
                Error::Status(format!(
                    "module {module:?}: active code hash is {} bytes, not 32",
                    entry.active_code_hash.len()
                ))
            })?)
        };
        if hashes.insert(module.clone(), hash).is_some() {
            return Err(Error::Status(format!(
                "module {module:?} is registered more than once"
            )));
        }
    }
    Ok(hashes)
}

/// The module's active code hash, or `None` for an admission that has not
/// reached its boundary.
pub async fn active_hash(client: &Client, module: &str) -> Result<Option<[u8; 32]>, Error> {
    active_hashes(client)
        .await?
        .remove(module)
        .ok_or_else(|| Error::Status(format!("module {module:?} is not registered")))
}

/// How long the node took over each question a resolve asks it.
#[derive(Default)]
pub struct Asked {
    /// the registry (`module_status`)
    pub status: Duration,
    /// the artifact blob
    pub fetch: Duration,
}

/// The module's view as its active deployment ships it; `asked` takes the
/// time each question of the node took.
pub async fn resolve(
    client: &Client,
    module: &str,
    asked: &mut Asked,
) -> Result<ViewSource, Error> {
    let started = Instant::now();
    let active = active_hash(client, module).await;
    asked.status = started.elapsed();
    let Some(hash) = active? else {
        return Ok(ViewSource::NotActivated);
    };
    let started = Instant::now();
    let loaded = view_artifact::load(client, hash).await;
    asked.fetch = started.elapsed();
    match loaded.map_err(Error::Artifact)? {
        None => Ok(ViewSource::Missing { hash }),
        Some(view) => Ok(ViewSource::Ready {
            hash,
            component: view.component,
            assets: Arc::new(view.assets),
        }),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use module_artifact::{ModuleArtifact, ViewArtifact};
    use std::sync::Mutex;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    /// A node that answers `module_status` with `status` and serves
    /// `artifact` under every blob digest asked for — so a registry naming
    /// a hash the bytes do not match is one test away. With `hold`, the
    /// blob is served only once it is notified.
    pub(crate) async fn node(
        status: serde_json::Value,
        artifact: Option<ModuleArtifact>,
        hold: Option<Arc<tokio::sync::Notify>>,
    ) -> Client {
        let deployment = FakeDeployment {
            status: Mutex::new(status),
            artifacts: Mutex::new(artifact.into_iter().collect()),
            by_digest: false,
            hold: Mutex::new(hold),
            hold_status: Mutex::new(None),
            held: tokio::sync::Notify::new(),
        };
        fake_node(Arc::new(deployment)).await
    }

    /// What a fake node serves, changeable under a running host: the
    /// registry reply, the artifacts (by their own digest when `by_digest`,
    /// else the first for any digest), and a one-shot hold on the next
    /// blob served.
    pub(crate) struct FakeDeployment {
        pub status: Mutex<serde_json::Value>,
        pub artifacts: Mutex<Vec<ModuleArtifact>>,
        pub by_digest: bool,
        /// One-shot holds: the next blob, or status, answer waits on it.
        pub hold: Mutex<Option<Arc<tokio::sync::Notify>>>,
        pub hold_status: Mutex<Option<Arc<tokio::sync::Notify>>>,
        /// Told each time an answer starts waiting on a hold.
        pub held: tokio::sync::Notify,
    }

    impl FakeDeployment {
        pub(crate) fn serving(module: &str, artifact: &ModuleArtifact) -> Arc<Self> {
            Arc::new(Self {
                status: Mutex::new(status_naming(module, &artifact.hash())),
                artifacts: Mutex::new(vec![artifact.clone()]),
                by_digest: true,
                hold: Mutex::new(None),
                hold_status: Mutex::new(None),
                held: tokio::sync::Notify::new(),
            })
        }

        /// The registry now names `artifact` as `module`'s active code,
        /// and the blob store has it.
        pub(crate) fn deploy(&self, module: &str, artifact: &ModuleArtifact) {
            *self.status.lock().unwrap() = status_naming(module, &artifact.hash());
            self.artifacts.lock().unwrap().push(artifact.clone());
        }
    }

    pub(crate) fn status_naming(module: &str, hash: &[u8]) -> serde_json::Value {
        serde_json::json!({"module_status": {"modules": [
            {"module_id": module, "active_code_hash": hash, "pending": null,
             "history": [{"height": 7, "code_hash": hash}]}
        ]}})
    }

    pub(crate) async fn fake_node(deployment: Arc<FakeDeployment>) -> Client {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                // each request on its own: a held blob leaves the status
                // answering, as a node does
                let deployment = deployment.clone();
                tokio::spawn(async move {
                    let mut request = vec![0u8; 4096];
                    let read = socket.read(&mut request).await.unwrap();
                    let head = String::from_utf8_lossy(&request[..read]).into_owned();
                    let route = head.split(' ').nth(1).unwrap_or("").to_owned();
                    let (status_line, body) = if route == "/v1/query" {
                        let hold = deployment.hold_status.lock().unwrap().take();
                        if let Some(hold) = hold {
                            deployment.held.notify_one();
                            hold.notified().await;
                        }
                        let status = deployment.status.lock().unwrap().clone();
                        ("200 OK", status.to_string().into_bytes())
                    } else if let Some(digest) = route.strip_prefix("/v1/files/blob/") {
                        let hold = deployment.hold.lock().unwrap().take();
                        if let Some(hold) = hold {
                            deployment.held.notify_one();
                            hold.notified().await;
                        }
                        let artifacts = deployment.artifacts.lock().unwrap();
                        let served = if deployment.by_digest {
                            artifacts.iter().find(|artifact| {
                                crate::backend::hex_encode(&artifact.hash()) == digest
                            })
                        } else {
                            artifacts.first()
                        };
                        match served {
                            Some(artifact) => ("200 OK", artifact.encode()),
                            None => ("404 Not Found", Vec::new()),
                        }
                    } else {
                        ("404 Not Found", Vec::new())
                    };
                    let response = format!(
                        "HTTP/1.1 {status_line}\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                        body.len()
                    );
                    socket.write_all(response.as_bytes()).await.unwrap();
                    let _ = socket.write_all(&body).await;
                    let _ = socket.shutdown().await;
                });
            }
        });
        Client::new(&origin).unwrap()
    }

    fn status_of(hash: &[u8]) -> serde_json::Value {
        serde_json::json!({"module_status": {"modules": [
            {"module_id": "files", "active_code_hash": hash, "pending": null,
             "history": [{"height": 7, "code_hash": hash}]},
            {"module_id": "chat", "active_code_hash": [], "pending": null, "history": []}
        ]}})
    }

    fn with_view() -> ModuleArtifact {
        ModuleArtifact {
            component: vec![1, 2, 3],
            index: None,
            view: Some(ViewArtifact {
                component: vec![4, 5, 6],
                assets: [("icons/action.svg".to_owned(), b"<svg/>".to_vec())].into(),
            }),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn an_activated_deployment_with_a_view_is_ready() {
        let artifact = with_view();
        let client = node(status_of(&artifact.hash()), Some(artifact.clone()), None).await;
        assert_eq!(
            resolve(&client, "files", &mut Asked::default())
                .await
                .unwrap(),
            ViewSource::Ready {
                hash: artifact.hash(),
                component: vec![4, 5, 6],
                assets: Arc::new(artifact.view.unwrap().assets),
            }
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_verified_deployment_without_a_view_is_missing_not_a_fallback() {
        let artifact = ModuleArtifact::component(vec![1, 2, 3]);
        let client = node(status_of(&artifact.hash()), Some(artifact.clone()), None).await;
        assert_eq!(
            resolve(&client, "files", &mut Asked::default())
                .await
                .unwrap(),
            ViewSource::Missing {
                hash: artifact.hash()
            }
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn bytes_that_do_not_hash_to_the_active_code_fail() {
        let client = node(status_of(&[7; 32]), Some(with_view()), None).await;
        assert!(matches!(
            resolve(&client, "files", &mut Asked::default()).await,
            Err(Error::Artifact(view_artifact::Error::HashMismatch))
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_fetch_that_fails_is_an_error() {
        let client = node(status_of(&[7; 32]), None, None).await;
        assert!(matches!(
            resolve(&client, "files", &mut Asked::default()).await,
            Err(Error::Artifact(view_artifact::Error::Transport(_)))
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn an_admission_before_its_boundary_is_not_activated() {
        let client = node(status_of(&[7; 32]), Some(with_view()), None).await;
        assert_eq!(
            resolve(&client, "chat", &mut Asked::default())
                .await
                .unwrap(),
            ViewSource::NotActivated
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_registry_the_reader_does_not_understand_is_an_error() {
        for (case, status) in [
            ("wrong-length hash", status_of(&[7u8; 31])),
            (
                "unknown field",
                serde_json::json!({"module_status": {"modules": [
                    {"module_id": "files", "active_code_hash": vec![7u8; 32], "pending": null,
                     "history": [], "extra": 1}
                ]}}),
            ),
            (
                "another reply",
                serde_json::json!({"armed_at": {"swaps": []}}),
            ),
            (
                "registered twice",
                serde_json::json!({"module_status": {"modules": [
                    {"module_id": "files", "active_code_hash": vec![7u8; 32], "pending": null, "history": []},
                    {"module_id": "files", "active_code_hash": vec![8u8; 32], "pending": null, "history": []}
                ]}}),
            ),
            (
                "not registered",
                serde_json::json!({"module_status": {"modules": []}}),
            ),
        ] {
            let client = node(status, Some(with_view()), None).await;
            assert!(
                matches!(
                    resolve(&client, "files", &mut Asked::default()).await,
                    Err(Error::Status(_))
                ),
                "{case}"
            );
        }
    }
}
