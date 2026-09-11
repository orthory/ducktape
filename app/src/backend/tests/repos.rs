use super::*;

#[test]
fn highlight_ranges_hold_char_boundaries_on_real_sources() {
    // this repo's own sources carry the multibyte punctuation ('—', '·', '→')
    // an ASCII probe never exercises; a syntect range that split a UTF-8 char
    // would make `code_surface`'s `line[range]` panic inside the live view.
    let rust = include_str!("../forge.rs");
    let toml = include_str!("../../../Cargo.toml");
    for (path, source) in [("forge.rs", rust), ("Cargo.toml", toml)] {
        let mut stream = iced::highlighter::Stream::new(&iced::highlighter::Settings {
            theme: code_theme(true),
            token: code_token(path),
        });
        for line in source.lines() {
            for (range, _highlight) in stream.highlight_line(line) {
                let _ = line[range].to_string();
            }
            stream.commit();
        }
    }
}

#[test]
fn forge_code_tokens_follow_the_path_and_rust_really_colors() {
    assert_eq!(code_token("src/main.rs"), "rs");
    assert_eq!(code_token("a/b/query.SQL"), "sql");
    assert_eq!(code_token("Makefile"), "makefile");
    assert_eq!(code_token(".gitignore"), "gitignore");
    // the stream really tokenizes: one rust line yields more than one ink,
    // and an unknown token degrades to plain text (uniform ink), never an
    // error — exactly the old single-ink reading.
    let colors = |token: &str| -> std::collections::BTreeSet<String> {
        let mut stream = iced::highlighter::Stream::new(&iced::highlighter::Settings {
            theme: code_theme(true),
            token: token.into(),
        });
        stream
            .highlight_line("fn main() { let answer = 42; }")
            .map(|(_, highlight)| format!("{:?}", highlight.color()))
            .collect()
    };
    assert!(
        colors("rs").len() > 1,
        "rust source highlights with more than one color"
    );
    assert_eq!(
        colors("no-such-language").len(),
        1,
        "an unknown token is plain text in one ink"
    );
}

#[test]
fn merge_builder_produces_the_cas_commit_and_its_minimal_pack() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = git2::Repository::init_bare(dir.path()).unwrap();
    let base = mirror_commit(&mirror, None, &[("a.txt", "base\n"), ("b.txt", "keep\n")]);
    let ours = mirror_commit(
        &mirror,
        Some(base),
        &[("a.txt", "ours\n"), ("b.txt", "keep\n")],
    );
    let theirs = mirror_commit(
        &mirror,
        Some(base),
        &[("a.txt", "base\n"), ("b.txt", "theirs\n")],
    );

    let build = merge_against_mirror(&mirror, ours, theirs, "Merge pull request #1").unwrap();
    let MergeBuild::Clean { merge_oid, pack } = build else {
        panic!("disjoint edits must merge cleanly");
    };

    // land the pack in the mirror and read the merge commit back out —
    // exactly what a validator does after the blob fan-out.
    let odb = mirror.odb().unwrap();
    let mut writepack = odb.packwriter().unwrap();
    std::io::Write::write_all(&mut writepack, &pack).unwrap();
    writepack.commit().unwrap();
    let merged = mirror
        .find_commit(git2::Oid::from_str(&merge_oid).unwrap())
        .unwrap();
    let parents: Vec<git2::Oid> = merged.parent_ids().collect();
    assert_eq!(parents, vec![ours, theirs], "target first, source second");
    let tree = merged.tree().unwrap();
    let read = |path: &str| {
        let entry = tree.get_path(Path::new(path)).unwrap();
        String::from_utf8(mirror.find_blob(entry.id()).unwrap().content().to_vec()).unwrap()
    };
    assert_eq!(read("a.txt"), "ours\n");
    assert_eq!(read("b.txt"), "theirs\n");
}

#[test]
fn merge_builder_reports_conflicts_and_builds_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = git2::Repository::init_bare(dir.path()).unwrap();
    let base = mirror_commit(&mirror, None, &[("a.txt", "base\n")]);
    let ours = mirror_commit(&mirror, Some(base), &[("a.txt", "ours\n")]);
    let theirs = mirror_commit(&mirror, Some(base), &[("a.txt", "theirs\n")]);

    let build = merge_against_mirror(&mirror, ours, theirs, "Merge pull request #2").unwrap();
    let MergeBuild::Conflicts(paths) = build else {
        panic!("competing edits must conflict");
    };
    assert_eq!(paths, vec!["a.txt".to_string()]);
}

/// A WEB PICTURE IS ONE CAPPED GET. The bytes come back as served; a
/// response that announces more than the viewer takes, one that streams more
/// than it announced (or announced nothing), and one without a body to show
/// all come back as `None` — the image keeps its alt text. This drives the
/// GET behind the host gate: the gate itself refuses loopback, which is
/// where a test server lives.
#[tokio::test(flavor = "current_thread")]
async fn a_web_picture_is_one_capped_get() {
    use super::super::picture::MAX_PICTURE_BYTES;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0u8; 2048];
            let read = socket.read(&mut request).await.unwrap();
            let head = String::from_utf8_lossy(&request[..read]).into_owned();
            let route = head.split(' ').nth(1).unwrap_or("").to_owned();
            let (status, length, body_len): (&str, Option<usize>, usize) = match route.as_str() {
                "/small" => ("200 OK", Some(9), 9),
                "/announced-huge" => ("200 OK", Some(MAX_PICTURE_BYTES + 1), 0),
                "/streamed-huge" => ("200 OK", None, MAX_PICTURE_BYTES + 1),
                _ => ("404 Not Found", Some(0), 0),
            };
            let mut response = format!("HTTP/1.1 {status}\r\nConnection: close\r\n");
            if let Some(length) = length {
                response.push_str(&format!("Content-Length: {length}\r\n"));
            }
            response.push_str("\r\n");
            socket.write_all(response.as_bytes()).await.unwrap();
            let body = match route.as_str() {
                "/small" => b"PNG-bytes".to_vec(),
                _ => vec![0u8; body_len],
            };
            let _ = socket.write_all(&body).await;
            let _ = socket.shutdown().await;
        }
    });
    assert_eq!(
        fetch_picture_bytes(reqwest::Url::parse(&format!("{origin}/small")).unwrap())
            .await
            .as_deref(),
        Some(&b"PNG-bytes"[..]),
        "a picture under the cap comes back as served"
    );
    assert!(
        fetch_picture_bytes(reqwest::Url::parse(&format!("{origin}/announced-huge")).unwrap())
            .await
            .is_none(),
        "an announced length past the cap is refused before the body"
    );
    assert!(
        fetch_picture_bytes(reqwest::Url::parse(&format!("{origin}/streamed-huge")).unwrap())
            .await
            .is_none(),
        "a body that streams past the cap is refused mid-stream"
    );
    assert!(
        fetch_picture_bytes(reqwest::Url::parse(&format!("{origin}/missing")).unwrap())
            .await
            .is_none(),
        "a miss has no picture"
    );
}

/// A README NAMES THE URL, THE READER'S MACHINE MAKES THE REQUEST. The host
/// gate refuses the machine itself, its link (cloud metadata), nothing and
/// everyone — as an IP literal or as a name that resolves there — and keeps
/// a LAN address, which is where a team's forge lives.
#[tokio::test(flavor = "current_thread")]
async fn a_web_picture_never_points_the_reader_at_its_own_machine() {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    let blocked = [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V4(Ipv4Addr::BROADCAST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
        IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped()),
        IpAddr::V6("fe80::1".parse().unwrap()),
    ];
    for ip in blocked {
        assert!(blocked_picture_host(ip), "{ip} is not a picture's address");
    }
    let allowed = [
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7)),
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)),
        IpAddr::V4(Ipv4Addr::new(140, 82, 112, 3)),
        IpAddr::V6("2606:4700::1".parse().unwrap()),
    ];
    for ip in allowed {
        assert!(!blocked_picture_host(ip), "{ip} may serve a picture");
    }
    // The gate end to end, without a socket: a literal and the name that
    // resolves to the machine are refused before any request is made.
    assert!(
        web_picture_bytes("http://127.0.0.1:9/logo.png")
            .await
            .is_none(),
        "a loopback literal is refused"
    );
    assert!(
        web_picture_bytes("http://localhost:9/logo.png")
            .await
            .is_none(),
        "a name resolving to loopback is refused"
    );
    assert!(
        web_picture_bytes("ftp://example.com/logo.png")
            .await
            .is_none(),
        "only a web URL is fetched"
    );
}
