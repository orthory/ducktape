//! The policy selector maps CLI flags to the right `AuthPolicy` variant, and
//! `--genesis-set` reads the PUBLIC valset out of a real `network.toml`.

use coordinator_bin::select_policy;
use nat_traversal::AuthPolicy;

use commonware_cryptography::{ed25519, Signer as _};

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[test]
fn flags_select_the_expected_policy() {
    // Legacy fully-open.
    let anon = select_policy(&["--allow-anonymous".into()]).unwrap();
    assert!(matches!(anon, AuthPolicy::Open { require_pop: false }));

    // Deployed default: public + proof-of-possession.
    let default = select_policy(&[]).unwrap();
    assert!(matches!(default, AuthPolicy::Open { require_pop: true }));
}

#[test]
fn genesis_set_pins_the_validators_from_network_toml() {
    // Two real genesis pubkeys.
    let a = ed25519::PrivateKey::from_seed(1).public_key();
    let b = ed25519::PrivateKey::from_seed(2).public_key();

    // A network.toml-shaped fixture with extra fields the coordinator ignores.
    let toml = format!(
        "chain_id = \"ducktape#deadbeef\"\n\
         scheme = \"ed25519\"\n\
         coordination = \"private\"\n\
         validators = [\"{}\", \"{}\"]\n\
         bootstrap = []\n",
        hex(a.as_ref()),
        hex(b.as_ref()),
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("network.toml");
    std::fs::write(&path, toml).unwrap();

    let policy = select_policy(&["--genesis-set".into(), path.to_str().unwrap().into()]).unwrap();
    match policy {
        AuthPolicy::Private { genesis_set } => {
            assert_eq!(genesis_set.len(), 2);
            assert!(genesis_set.contains(&a));
            assert!(genesis_set.contains(&b));
        }
        other => panic!("expected Private, got {other:?}"),
    }
}

#[test]
fn genesis_set_flag_beats_allow_anonymous_is_not_the_precedence_but_anon_wins() {
    // Documented precedence: --allow-anonymous short-circuits before
    // --genesis-set, so an operator who passes both gets the (explicit) open
    // policy and never has to have a valid file on disk.
    let policy = select_policy(&[
        "--allow-anonymous".into(),
        "--genesis-set".into(),
        "/does/not/exist.toml".into(),
    ])
    .unwrap();
    assert!(matches!(policy, AuthPolicy::Open { require_pop: false }));
}

#[test]
fn genesis_set_missing_file_is_a_hard_error() {
    let err = select_policy(&["--genesis-set".into(), "/no/such/network.toml".into()]).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}
