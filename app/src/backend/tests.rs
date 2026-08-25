use ::chat;
use ::forge;
use ::node;

use commonware_cryptography::{Signer as _, ed25519};
use iced::futures::StreamExt as _;

use super::*;
use crate::{ForgeTab, MembersFilter, MessageAction, ShellTab};

mod review;
mod repos;
mod docs;
mod messages;
mod status;
mod shell;
mod wire;

fn stage(staged: Vec<ForgeDraftComment>, line: &str, body: &str) -> Vec<ForgeDraftComment> {
    stage_forge_comment(
        staged,
        "src/main.rs".into(),
        line.into(),
        "new".into(),
        body.into(),
    )
}

fn alpha_of(background: iced::Background) -> f32 {
    let iced::Background::Color(color) = background else {
        panic!("a depth role paints a flat colour");
    };
    color.a
}

/// One stubbed lane of the workspace search: the substring that names it in a
/// raw request, the search LEG it belongs to, and the reply. An empty leg name
/// marks a FOLLOW-UP — a request a leg can only issue after its own first reply
/// (forge's per-repo tracker read, pages' title lookup), so it can never be in
/// flight at first contact and is not counted as overlap.
///
/// FIRST MATCH WINS, SO A FOLLOW-UP GOES ABOVE THE LANE IT SHARES A ROUTE WITH.
/// `list_pages` is `/v1/index/pages/view` too — the same route as the page
/// search — so on the substring alone the title lookup would be served the
/// search's own reply and read as a second `pages` arrival. Matching the query
/// discriminant first is what keeps "a request this stub does not model panics
/// by name" true rather than nearly true.
///
/// The chat lane answers with no hits on purpose: a `MsgRow` is seventeen wire
/// fields and nothing here turns on its contents. The other five carry one
/// matching row each, which is what pins the row order.
const SEARCH_LANES: &[(&str, &str, &str)] = &[
    ("/v1/index/chat/view", "chat", r#"{"hits":[]}"#),
    (
        "list_pages",
        "",
        r#"{"pages":{"pages":[{"id":"page-1","title":"The needle page"}],"has_more":false}}"#,
    ),
    (
        "/v1/index/pages/view",
        "pages",
        r#"{"hits":[{"block_id":"block-1","page_id":"page-1","parent":"page-1","kind":"paragraph","text":"needle in a page","height":1,"time":1}]}"#,
    ),
    (
        "list_repos",
        "forge",
        r#"{"repos":[{"name":"needle-repo","head":"0000000000000000000000000000000000000000"}]}"#,
    ),
    (
        "list_items",
        "",
        r#"{"items":[{"number":7,"kind":"issue","title":"needle issue","state":"open","author":"system","created_at":1,"updated_at":1}]}"#,
    ),
    (
        "/v1/files/grep",
        "files",
        r#"{"hits":[{"path":"src/needle.rs","line":3,"text":"a needle here"}]}"#,
    ),
    (
        "/v1/index/tasks/view",
        "tasks",
        r#"{"tasks":{"tasks":[{"title":"needle task","task_id":"task-1","created_by":"user:aa","updated_height":2}]}}"#,
    ),
    (
        "pending_runs",
        "runs",
        r#"{"pending_runs":[{"run_id":"needle-run","agent_id":"agent-1","created_at":1,"channel_id":"c1"}]}"#,
    ),
    ("recent_runs", "runs", r#"{"recent_runs":[]}"#),
];

/// The lane one raw HTTP request belongs to. `None` = a request this stub does
/// not model, which is a test bug, not a product one.
fn search_lane_of(request: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    SEARCH_LANES
        .iter()
        .find(|(mark, ..)| request.contains(mark))
}

/// Which REQUESTS of a workspace search were in flight AT THE SAME MOMENT, each
/// tagged with the leg it belongs to, as seen by the stub node below.
///
/// Requests, not legs — a set of leg names cannot see a leg serializing its own
/// round trips (tasks reads three status pages, runs reads two queries), and
/// "the six legs overlapped" stays true while the work inside them is a chain.
/// Counting arrivals makes the multiplicity part of the answer.
#[derive(Default)]
struct FanOutWatch {
    waiting: Vec<String>,
    /// The arrivals as they stood when the stub let them through — the answer
    /// the test asserts on. Empty until then.
    overlapped: Vec<String>,
    released: bool,
}

impl FanOutWatch {
    /// Record one arrival; true when it is the one that completes the wave. An
    /// arrival AFTER release is not recorded: it is by definition a request that
    /// was waiting on something, which is the failure being measured.
    fn arrive(&mut self, leg: &str, requests: usize) -> bool {
        if self.released {
            return false;
        }
        self.waiting.push(leg.to_string());
        self.waiting.len() >= requests
    }

    /// Let everyone through and freeze the report.
    fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        self.overlapped = self.waiting.clone();
        self.overlapped.sort();
    }
}

/// A stub node that ANSWERS NOTHING until a workspace search has `requests`
/// round trips in flight at once, then releases them together. That overlap is
/// the only thing observable from outside the process, and it is the whole
/// guarantee: a request that waits on another request's reply — a serial chain,
/// a nested `.await` inside the join, a helper that folds two legs into one
/// future, a leg walking its own pages one at a time — cannot be in flight
/// beside what it waits on, so the wave never completes and the stub reports
/// exactly what did overlap.
///
/// [`FanOutWatch::overlapped`] is filled ONCE, at release, and requests that
/// arrive after it are not counted — the report is what overlapped, not what
/// ever arrived. Recording every arrival instead makes this stub pass the very
/// break it exists to catch: a leg held back behind another leg's reply lands
/// the moment the rest are let go, and a set that keeps filling then reads as a
/// full fan-out.
///
/// A grep of the join's TEXT cannot see any of this, which is why the pin this
/// replaced could be broken while staying green.
///
/// THE ESCAPE FROM A WEDGE IS AN EVENT, NOT A CLOCK. A serialized search never
/// completes the wave, so its held requests run out `RpcClient`'s own 30 s
/// ceiling and reqwest drops the connection — and that FIN is what this stub
/// waits on beside the release. The first hang-up freezes the report at what
/// had genuinely overlapped and lets the rest go, so a broken fan-out FAILS
/// with the truth in the message instead of hanging the suite. The passing path
/// never touches either seam: nine loopback requests overlap in milliseconds
/// and release on the ninth ARRIVAL, so no duration is load-bearing anywhere
/// here.
async fn node_that_answers_only_a_full_fan_out(
    requests: usize,
    refused: &'static [&'static str],
    watch: std::sync::Arc<Mutex<FanOutWatch>>,
) -> String {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    /// The request is in hand once the body reaches its declared length.
    fn request_is_complete(request: &[u8]) -> bool {
        let text = String::from_utf8_lossy(request);
        let Some((head, body)) = text.split_once("\r\n\r\n") else {
            return false;
        };
        let declared = head
            .to_lowercase()
            .lines()
            .find_map(|line| {
                line.strip_prefix("content-length:")?
                    .trim()
                    .parse::<usize>()
                    .ok()
            })
            .unwrap_or(0);
        body.len() >= declared
    }

    /// The held request's client gave up and closed the socket — the only
    /// event a stub holding a reply can observe when the wave will never
    /// complete. `Ok(0)` is the FIN; an error is the same fact, harder.
    async fn hung_up(stream: &mut tokio::net::TcpStream) {
        let mut ignored = [0u8; 1];
        while let Ok(read) = stream.read(&mut ignored).await {
            if read == 0 {
                return;
            }
        }
    }

    let (release, _) = tokio::sync::watch::channel(false);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the stub node");
    let origin = format!("http://{}", listener.local_addr().expect("stub address"));
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let watch = watch.clone();
            let release = release.clone();
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut chunk = [0u8; 2048];
                while let Ok(read) = stream.read(&mut chunk).await {
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&chunk[..read]);
                    if request_is_complete(&request) {
                        break;
                    }
                }
                let request = String::from_utf8_lossy(&request).to_string();
                let (_, leg, reply) = search_lane_of(&request).unwrap_or_else(|| {
                    panic!(
                        "the workspace search touched a lane this stub does not \
                         model, so it costs a round trip nobody accounted for: \
                         {request}"
                    )
                });
                let counts = !leg.is_empty();
                if counts {
                    let completes_the_wave =
                        watch.lock().expect("stub watch").arrive(leg, requests);
                    if completes_the_wave {
                        watch.lock().expect("stub watch").release();
                        let _ = release.send(true);
                    }
                    let mut open = release.subscribe();
                    let opened = async {
                        while !*open.borrow_and_update() {
                            let _ = open.changed().await;
                        }
                    };
                    tokio::select! {
                        () = opened => {}
                        () = hung_up(&mut stream) => {
                            // Nobody is coming: this request's client already
                            // walked away. Freeze the report at what did
                            // overlap and let the others answer into the void.
                            watch.lock().expect("stub watch").release();
                            let _ = release.send(true);
                        }
                    }
                }
                let refuse = refused.contains(leg);
                let (status, reply) = match refuse {
                    true => ("503 Service Unavailable", "the module is not answering"),
                    false => ("200 OK", *reply),
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    origin
}

/// A node that serves `GET /v1/status` EXACTLY ONCE and answers `500` to every
/// later ask for it. `/v1/peers` answers every time — the pin is on the status
/// document, not on the peer sample.
///
/// This is the whole point of the fixture: a loader that reads the chain twice
/// to fill one card cannot get away with it here, whichever field it takes from
/// whichever read. Counting reads is the only pin that survives a rename —
/// #1017's first round asserted identifier names instead, and a reviewer put
/// the literal second `client.status()` back with every name intact and all
/// 272 tests still green.
async fn node_that_serves_its_status_once(status_body: &'static str) -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the stub node");
    let origin = format!("http://{}", listener.local_addr().expect("stub address"));
    let status_reads = std::sync::Arc::new(AtomicUsize::new(0));
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut request = Vec::new();
            let mut chunk = [0u8; 2048];
            // Both routes are bodyless GETs, so the request is in hand as soon
            // as the head is.
            while let Ok(read) = stream.read(&mut chunk).await {
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let asked_for_status = String::from_utf8_lossy(&request).contains("/v1/status");
            let already_served =
                asked_for_status && status_reads.fetch_add(1, Ordering::SeqCst) > 0;
            let (code, body) = match (asked_for_status, already_served) {
                (true, false) => ("200 OK", status_body),
                (true, true) => (
                    "500 Internal Server Error",
                    "this node answers /v1/status once",
                ),
                (false, _) => ("200 OK", r#"{"peers":[]}"#),
            };
            let response = format!(
                "HTTP/1.1 {code}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });
    origin
}

/// The next update that says something happened.
///
/// THE TIP ARRIVES FIRST, EVERY BLOCK. The node sends the heartbeat on the
/// block wake and only THEN catches its topics up (`crates/noded/src/stream.rs`,
/// the `block_rx` arm), so the head for block N is on the wire before N's ops
/// are. A test that asserts on `live.next()` directly is asserting on that
/// heartbeat, not on its own submit.
async fn next_change(
    live: &mut iced::futures::stream::BoxStream<'static, LiveUpdate>,
) -> LiveUpdate {
    loop {
        let update = live.next().await.expect("live stream ended");
        if update.kind != crate::LiveKind::Tip {
            return update;
        }
        // AND WHILE WE ARE HOLDING A REAL ONE, PIN IT HERE. The unit test below
        // builds its own tip, so it can only speak for `live_update` — it stays
        // green if the stream's arm starts asking for a load. These are the
        // tips the node actually sent, decoded by the real client, so this is
        // the assertion that binds the arm.
        assert!(update.height > 0, "a tip carries the head it was sent with");
        assert!(
            !update.load_chat && !update.load_pages,
            "a tip must not trigger a load — that is a 1 Hz poll on an idle chain"
        );
    }
}

/// drain the live event stream until the index has folded the block at
/// `min_height` — the system's own commit signal, never a timed poll.
async fn wait_for_block(
    live: &mut iced::futures::stream::BoxStream<'static, LiveUpdate>,
    min_height: i64,
) {
    loop {
        let update = live.next().await.expect("live stream ended");
        let folded = matches!(update.kind, crate::LiveKind::Chat | crate::LiveKind::Pages);
        if folded && update.height >= min_height {
            return;
        }
    }
}

async fn submit_test(
    rpc: &RpcClient,
    signer: &ed25519::PrivateKey,
    sequence: u64,
    target: &str,
    payload: Vec<u8>,
) {
    let frame = node::encode_frame(
        signer,
        sequence,
        &sdk::Msg {
            target: target.into(),
            payload,
        },
    );
    rpc.submit_frame(frame).await.unwrap();
}

/// One commit in `repo` holding exactly `files`, on top of `parent`.
fn mirror_commit(
    repo: &git2::Repository,
    parent: Option<git2::Oid>,
    files: &[(&str, &str)],
) -> git2::Oid {
    let mut tree = repo.treebuilder(None).unwrap();
    for (path, contents) in files {
        let blob = repo.blob(contents.as_bytes()).unwrap();
        tree.insert(path, blob, 0o100644).unwrap();
    }
    let tree = repo.find_tree(tree.write().unwrap()).unwrap();
    let signature = git2::Signature::now("mule", "mule@localhost").unwrap();
    let parents: Vec<git2::Commit> = parent
        .map(|oid| vec![repo.find_commit(oid).unwrap()])
        .unwrap_or_default();
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    repo.commit(None, &signature, &signature, "mule", &tree, &parent_refs)
        .unwrap()
}

/// A node whose page SEARCH answers and whose page LIST refuses — the exact
/// split the title join has to survive. Answers one request per connection and
/// closes, so the two views of a search never share a socket. Returns its
/// origin.
async fn node_with_a_broken_page_list() -> String {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    /// The request is in hand once the body reaches its declared length.
    fn request_is_complete(request: &[u8]) -> bool {
        let text = String::from_utf8_lossy(request);
        let Some((head, body)) = text.split_once("\r\n\r\n") else {
            return false;
        };
        let declared = head
            .to_lowercase()
            .lines()
            .find_map(|line| {
                line.strip_prefix("content-length:")?
                    .trim()
                    .parse::<usize>()
                    .ok()
            })
            .unwrap_or(0);
        body.len() >= declared
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the stub node");
    let origin = format!("http://{}", listener.local_addr().expect("stub address"));
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut request = Vec::new();
            let mut chunk = [0u8; 2048];
            while let Ok(read) = stream.read(&mut chunk).await {
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if request_is_complete(&request) {
                    break;
                }
            }
            // Both views POST to `/v1/index/pages/view`; only the body says
            // which one this is. Shapes copied from the live demo node.
            let asked_for_the_index = String::from_utf8_lossy(&request).contains("list_pages");
            let (status, body) = match asked_for_the_index {
                true => ("500 Internal Server Error", "pages index unavailable"),
                false => (
                    "200 OK",
                    r#"{"hits":[{"block_id":"block-1","page_id":"page-1","parent":"page-1","kind":"paragraph","text":"Tail paragraph after the list","height":1,"time":1}]}"#,
                ),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });
    origin
}

/// One function's body out of a backend module: from its declaration to the
/// first closing brace at column zero, which in fmt'd Rust is its own. Sliced
/// rather than scanned to the next `pub` — `pub(crate)` does not start with
/// `pub `, so that boundary silently runs a negative assertion over the rest
/// of the file and fails on some LATER function's read.
fn backend_fn<'a>(source: &'a str, declaration: &str) -> &'a str {
    source
        .split(declaration)
        .nth(1)
        .unwrap_or_else(|| panic!("{declaration} is declared"))
        .split("\n}\n")
        .next()
        .unwrap_or_else(|| panic!("{declaration} body"))
}

/// Every backend module's source, this test file excepted — the lane pins
/// above sweep the whole crate rather than the handful of files that happen to
/// hold a read today.
fn backend_sources() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/backend");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("the backend tree is readable")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "rs"))
        .filter(|path| path.file_name().is_some_and(|name| name != "tests.rs"))
        .map(|path| {
            let source = std::fs::read_to_string(&path).expect("a backend module reads");
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            (name, source)
        })
        .collect();
    out.sort();
    out
}

/// A stub node whose `/v1/index/pages/view` replies carry a SCRIPTED fold
/// watermark: one entry per request, the last one repeating forever. Returns
/// its origin plus the live request count, which is how the bound on
/// `await_fold` is observed rather than assumed.
///
/// One request per connection, then close — the same discipline as
/// `node_with_a_broken_page_list`, so no probe can share a socket with the
/// next and read a stale header off it.
async fn node_scripting_its_fold_watermark(
    script: Vec<Option<&'static str>>,
) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    let served = std::sync::Arc::new(AtomicUsize::new(0));
    let counter = served.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the stub node");
    let origin = format!("http://{}", listener.local_addr().expect("stub address"));
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut request = Vec::new();
            let mut chunk = [0u8; 2048];
            while let Ok(read) = stream.read(&mut chunk).await {
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if String::from_utf8_lossy(&request).contains("\r\n\r\n") {
                    break;
                }
            }
            let nth = counter.fetch_add(1, Ordering::SeqCst);
            let folded = script[nth.min(script.len() - 1)];
            let body = r#"{"threads":[]}"#;
            let watermark = folded
                .map(|tip| format!("x-ducktape-folded: {tip}\r\n"))
                .unwrap_or_default();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n{watermark}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });
    (origin, served)
}
