use std::process::Command;

fn command_output(out: &std::process::Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn coordinated_invite_persists_tunnel_bootstrap_without_direct_endpoint() {
    let dir = tempfile::tempdir().expect("tempdir");
    let founder = dir.path().join("founder");
    let friend = dir.path().join("friend");

    let init = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "init",
            "--name",
            "coordinated-default",
            "--dir",
            founder.to_str().expect("utf-8 founder dir"),
        ])
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "init failed:\n{}",
        command_output(&init)
    );

    let invite = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args(["invite", "--config"])
        .arg(founder.join("node.toml"))
        .output()
        .expect("run invite");
    assert!(
        invite.status.success(),
        "invite failed:\n{}",
        command_output(&invite)
    );
    let blob = String::from_utf8_lossy(&invite.stdout).trim().to_string();

    let join = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "join",
            &blob,
            "--dir",
            friend.to_str().expect("utf-8 friend dir"),
        ])
        .output()
        .expect("run join");
    assert!(
        join.status.success(),
        "join failed:\n{}",
        command_output(&join)
    );

    let bootstrap = friend.join("invite-wireguard.toml");
    assert!(
        bootstrap.exists(),
        "coordinated invite must persist the inviter's WireGuard bootstrap"
    );
    let text = std::fs::read_to_string(&bootstrap).expect("read invite-wireguard");
    assert!(
        text.contains("public_key"),
        "bootstrap must carry the inviter's WireGuard key:\n{text}"
    );
    assert!(
        text.contains("mesh_port"),
        "bootstrap must carry the inviter's overlay mesh port:\n{text}"
    );
    assert!(
        !text.contains("endpoint"),
        "coordinated bootstrap must not bake in a direct underlay endpoint:\n{text}"
    );
}
