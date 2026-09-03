//! git's push certificate (`git push --signed`), as consensus reads it.
//!
//! git writes the certificate itself (send-pack.c `generate_push_cert`):
//!
//! ```text
//! certificate version 0.1
//! pusher <signing key ident> <timestamp> <tz>
//! pushee <url>
//! nonce <nonce the server advertised>
//! [push-option …]
//!
//! <old sha1> <new sha1> <refname>
//! …
//! ```
//!
//! and signs exactly that text with the pusher's SSH key (`ssh-keygen -Y sign
//! -n git`). The smart-HTTP bridge puts text + signature on the op as
//! [`PushCert`]; every validator then checks, in this order: the SSHSIG
//! verifies for the key it embeds; the certificate's update list IS the op's
//! (a cert cannot be borrowed to authorize different moves); the nonce names
//! this repo. The nonce is `<chain id>/<repo>`; the chain half is checked by
//! the bridge (which knows its chain — `Env` carries none), the repo half
//! here. Freshness is not a concern: a certificate names exact old→new
//! moves, so a replay is a no-op CAS.

use crate::PushCert;
use crate::oid::Oid;
use crate::tracker_iface::RefUpdate;

const VERSION_LINE: &str = "certificate version 0.1";
const HEADS: &str = "refs/heads/";

/// the parsed certificate: what the pusher committed to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Certificate {
    pub nonce: String,
    pub updates: Vec<RefUpdate>,
}

/// the nonce a node advertises for `repo`: `<chain id>/<repo>`.
pub fn nonce(chain_id: &str, repo: &str) -> String {
    format!("{chain_id}/{repo}")
}

/// does `nonce` end in `/<repo>` — the half consensus can check.
pub fn nonce_names_repo(nonce: &str, repo: &str) -> bool {
    nonce
        .strip_suffix(repo)
        .is_some_and(|head| head.ends_with('/'))
}

/// the certificate text git would write for `updates` under `nonce` — the
/// shape the bridge's and forge's tests sign; git's own carries pusher/pushee
/// lines this parser skips.
pub fn certificate(nonce: &str, updates: &[RefUpdate]) -> Vec<u8> {
    let mut text = format!("{VERSION_LINE}\nnonce {nonce}\n\n");
    for update in updates {
        let hex = |oid: &Option<Vec<u8>>| match oid {
            Some(bytes) => bytes.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            None => "0".repeat(40),
        };
        text.push_str(&format!(
            "{} {} {HEADS}{}\n",
            hex(&update.prev_oid),
            hex(&update.new_oid),
            update.ref_name
        ));
    }
    text.into_bytes()
}

/// parse the signed text. Header lines other than `nonce` are skipped (git
/// adds pusher/pushee/push-option); every update line must name a branch.
pub fn parse(cert: &[u8]) -> Result<Certificate, String> {
    let text = std::str::from_utf8(cert).map_err(|_| "push certificate is not utf-8")?;
    let mut lines = text.lines();
    let versioned = lines.next() == Some(VERSION_LINE);
    if !versioned {
        return Err(format!(
            "push certificate does not start with {VERSION_LINE:?}"
        ));
    }
    let mut nonce = None;
    for line in lines.by_ref() {
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("nonce ") {
            nonce = Some(value.to_string());
        }
    }
    let Some(nonce) = nonce else {
        return Err("push certificate carries no nonce".into());
    };
    let mut updates = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        updates.push(update_line(line)?);
    }
    if updates.is_empty() {
        return Err("push certificate lists no ref updates".into());
    }
    Ok(Certificate { nonce, updates })
}

fn update_line(line: &str) -> Result<RefUpdate, String> {
    let mut parts = line.split(' ');
    let (Some(old), Some(new), Some(refname), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(format!(
            "push certificate line is not `<old> <new> <refname>`: {line:?}"
        ));
    };
    let Some(branch) = refname.strip_prefix(HEADS) else {
        return Err(format!(
            "push certificate moves a non-branch ref {refname:?}"
        ));
    };
    Ok(RefUpdate {
        ref_name: branch.to_string(),
        prev_oid: oid_field(old)?,
        new_oid: oid_field(new)?,
    })
}

/// a 40-hex sha1; the zero oid is "unborn"/"delete" (`None`).
fn oid_field(hex: &str) -> Result<Option<Vec<u8>>, String> {
    let oid =
        Oid::from_hex(hex).map_err(|_| format!("push certificate oid is not sha1 hex: {hex:?}"))?;
    Ok((!oid.is_zero()).then(|| oid.as_bytes().to_vec()))
}

/// the SSH key that signed `cert` for THIS push: the SSHSIG verifies for the
/// key it embeds, the certificate's updates equal `updates` as a set, and its
/// nonce names `repo`. The 32 raw ed25519 key bytes — a member key's form.
pub fn signer(cert: &PushCert, repo: &str, updates: &[RefUpdate]) -> Result<Vec<u8>, String> {
    let sig = keyscheme::sshsig::parse(&cert.sshsig)?;
    let verified = keyscheme::sshsig::verify_ed25519(
        &sig.pubkey,
        keyscheme::sshsig::GIT_SSH_NS,
        &cert.cert,
        &cert.sshsig,
    );
    if !verified {
        return Err("push certificate signature does not verify".into());
    }
    let certificate = parse(&cert.cert)?;
    if !nonce_names_repo(&certificate.nonce, repo) {
        return Err(format!(
            "push certificate nonce {:?} does not name repo {repo:?}",
            certificate.nonce
        ));
    }
    let same_moves = sorted(&certificate.updates) == sorted(updates);
    if !same_moves {
        return Err("push certificate does not list this push's ref updates".into());
    }
    Ok(sig.pubkey.to_vec())
}

fn sorted(updates: &[RefUpdate]) -> Vec<&RefUpdate> {
    let mut sorted: Vec<&RefUpdate> = updates.iter().collect();
    sorted.sort_by(|a, b| a.ref_name.cmp(&b.ref_name));
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyscheme::sshsig::{GIT_SSH_NS, dearmor};
    use keyscheme::testkit::{ssh_key, ssh_pubkey, sshsig};

    /// the same real `ssh-keygen -Y sign -n git` fixture keyscheme pins.
    const CERT: &str = "certificate version 0.1\npusher key::ssh-ed25519 AAAA 1756332000 +0000\npushee http://127.0.0.1:8844/forge/lab\nnonce chain-a/lab\n\n0000000000000000000000000000000000000000 ab5b1f3d5b7e3e0e0d33e2c6d1f6c2a7d3a7f1e2 refs/heads/main\n";
    const ARMORED: &str = "-----BEGIN SSH SIGNATURE-----\n\
U1NIU0lHAAAAAQAAADMAAAALc3NoLWVkMjU1MTkAAAAgJjhQt02r3vG8+pxaBdryKnexRC\n\
cULQqMrrcadzt/2iEAAAADZ2l0AAAAAAAAAAZzaGE1MTIAAABTAAAAC3NzaC1lZDI1NTE5\n\
AAAAQAkqyuC4rshUkBgUVsgAqGxBltLKRLcwdq5LAQn+2lCUmiUJWTsYTykmuaNO+cntB2\n\
ZYBzkWoVNWmNV5YTCuZwE=\n\
-----END SSH SIGNATURE-----\n";

    fn main_birth() -> RefUpdate {
        RefUpdate {
            ref_name: "main".into(),
            prev_oid: None,
            new_oid: Some(
                (0..20)
                    .map(|i| {
                        u8::from_str_radix(
                            &"ab5b1f3d5b7e3e0e0d33e2c6d1f6c2a7d3a7f1e2"[2 * i..2 * i + 2],
                            16,
                        )
                        .unwrap()
                    })
                    .collect(),
            ),
        }
    }

    #[test]
    fn gits_own_certificate_parses_and_names_its_signer() {
        let parsed = parse(CERT.as_bytes()).unwrap();
        assert_eq!(parsed.nonce, "chain-a/lab");
        assert_eq!(parsed.updates, vec![main_birth()]);
        let cert = PushCert {
            cert: CERT.as_bytes().to_vec(),
            sshsig: dearmor(ARMORED).unwrap(),
        };
        let key = signer(&cert, "lab", &[main_birth()]).unwrap();
        assert_eq!(
            key,
            keyscheme::sshsig::authorized_key(
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICY4ULdNq97xvPqcWgXa8ip3sUQnFC0KjK63Gnc7f9oh"
            )
            .unwrap()
        );
        assert!(
            signer(&cert, "other", &[main_birth()])
                .unwrap_err()
                .contains("nonce")
        );
        let mut moved = main_birth();
        moved.ref_name = "dev".into();
        assert!(
            signer(&cert, "lab", &[moved])
                .unwrap_err()
                .contains("ref updates")
        );
        let mut extra = vec![main_birth(), main_birth()];
        extra[1].ref_name = "feature".into();
        assert!(signer(&cert, "lab", &extra).is_err());
        let mut forged = cert.clone();
        forged.cert.push(b'\n');
        forged.cert.extend_from_slice(b"0000000000000000000000000000000000000000 ab5b1f3d5b7e3e0e0d33e2c6d1f6c2a7d3a7f1e2 refs/heads/dev\n");
        assert!(
            signer(&forged, "lab", &[main_birth()])
                .unwrap_err()
                .contains("does not verify")
        );
    }

    #[test]
    fn the_builder_writes_what_git_would_and_the_parser_refuses_the_rest() {
        let sk = ssh_key(9);
        let updates = vec![
            RefUpdate {
                ref_name: "feature/x".into(),
                prev_oid: Some(vec![1; 20]),
                new_oid: None,
            },
            main_birth(),
        ];
        let text = certificate(&nonce("chain-b", "lab"), &updates);
        assert!(
            std::str::from_utf8(&text)
                .unwrap()
                .starts_with("certificate version 0.1\nnonce chain-b/lab\n\n")
        );
        assert_eq!(parse(&text).unwrap().updates, updates);
        let cert = PushCert {
            sshsig: sshsig(&sk, GIT_SSH_NS, &text),
            cert: text.clone(),
        };
        let reordered: Vec<RefUpdate> = updates.iter().rev().cloned().collect();
        assert_eq!(
            signer(&cert, "lab", &reordered).unwrap(),
            ssh_pubkey(&sk),
            "order-free"
        );
        let under_ducktape = PushCert {
            sshsig: sshsig(&sk, keyscheme::sshsig::DUCKTAPE_SSH_NS, &text),
            cert: text,
        };
        assert!(
            signer(&under_ducktape, "lab", &updates).is_err(),
            "namespace `git` only"
        );

        assert!(parse(b"nope").unwrap_err().contains("certificate version"));
        assert!(
            parse(b"certificate version 0.1\npusher x\n\n")
                .unwrap_err()
                .contains("nonce")
        );
        assert!(
            parse(b"certificate version 0.1\nnonce a/b\n\n")
                .unwrap_err()
                .contains("no ref updates")
        );
        assert!(parse(b"certificate version 0.1\nnonce a/b\n\n0000000000000000000000000000000000000000 ab5b1f3d5b7e3e0e0d33e2c6d1f6c2a7d3a7f1e2 refs/tags/v1\n").unwrap_err().contains("non-branch"));
        assert!(
            parse(b"certificate version 0.1\nnonce a/b\n\nzz ab refs/heads/main\n")
                .unwrap_err()
                .contains("sha1 hex")
        );
        assert!(nonce_names_repo("chain/lab", "lab"));
        assert!(!nonce_names_repo("chainlab", "lab"));
        assert!(!nonce_names_repo("chain/lab", "ab"));
    }
}
