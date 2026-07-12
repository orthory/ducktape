//! Port allocation for a workspace's node: distinct free localhost ports that
//! avoid everything the registry has already committed.

use std::net::TcpListener;

use super::registry::{Ports, Registry};

/// grab a free localhost port by binding `:0` and reading the assignment back.
/// a local single-user TOCTOU window we accept — the node rebinds it moments
/// later. `used` avoids handing out the same port twice in one allocation.
pub(super) fn free_port(used: &[u16]) -> Result<u16, String> {
    for _ in 0..64 {
        let listener =
            TcpListener::bind("127.0.0.1:0").map_err(|err| format!("probe free port: {err}"))?;
        let port = listener.local_addr().map_err(|err| err.to_string())?.port();
        drop(listener);
        if !used.contains(&port) {
            return Ok(port);
        }
    }
    Err("could not find a free localhost port".into())
}

/// [`free_port`]'s UDP twin — the wireguard/invite listeners are UDP, and a
/// free TCP port says nothing about the same UDP port (a collision silently
/// disables the reachability plane). same TOCTOU window, same dedup.
fn free_udp_port(used: &[u16]) -> Result<u16, String> {
    for _ in 0..64 {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0")
            .map_err(|err| format!("probe free udp port: {err}"))?;
        let port = socket.local_addr().map_err(|err| err.to_string())?.port();
        drop(socket);
        if !used.contains(&port) {
            return Ok(port);
        }
    }
    Err("could not find a free localhost udp port".into())
}

/// distinct free ports, avoiding every port already recorded in the
/// registry — a stopped workspace's ports are still ITS ports; handing them to
/// a new workspace would collide the moment both run.
pub(super) fn allocate_ports(reserved: &[u16]) -> Result<Ports, String> {
    let mut used = reserved.to_vec();
    let listen = free_port(&used)?;
    used.push(listen);
    let http = free_port(&used)?;
    used.push(http);
    let rpc = free_port(&used)?;
    used.push(rpc);
    let wireguard = free_udp_port(&used)?;
    used.push(wireguard);
    let invite = free_udp_port(&used)?;
    Ok(Ports {
        listen,
        http,
        rpc,
        wireguard: Some(wireguard),
        invite: Some(invite),
    })
}

/// every port the registry has already committed to a workspace.
pub(super) fn reserved_ports(reg: &Registry) -> Vec<u16> {
    reg.workspaces
        .iter()
        .flat_map(|w| {
            [
                Some(w.ports.listen),
                Some(w.ports.http),
                Some(w.ports.rpc),
                w.ports.wireguard,
                w.ports.invite,
            ]
        })
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::registry::Workspace;
    use super::*;

    #[test]
    fn allocated_ports_avoid_reserved() {
        let reserved = [40000u16, 40001, 40002];
        let p = allocate_ports(&reserved).unwrap();
        let got = [
            p.listen,
            p.http,
            p.rpc,
            p.wireguard.expect("wireguard port"),
            p.invite.expect("invite port"),
        ];
        for port in got {
            assert!(!reserved.contains(&port));
        }
        for (idx, port) in got.iter().enumerate() {
            assert!(
                !got[..idx].contains(port),
                "allocated duplicate port {port}"
            );
        }
    }

    #[test]
    fn reserved_ports_includes_reachability_ports() {
        let reg = Registry {
            version: 1,
            active: None,
            workspaces: vec![Workspace {
                id: "team".into(),
                name: "Team".into(),
                chain_id: "chain".into(),
                pubkey: "key".into(),
                founder: true,
                member: true,
                ports: Ports {
                    listen: 40000,
                    http: 40001,
                    rpc: 40002,
                    wireguard: Some(40003),
                    invite: Some(40004),
                },
            }],
            mnemonic_confirmed: false,
        };
        let got = reserved_ports(&reg);
        for port in [40000, 40001, 40002, 40003, 40004] {
            assert!(got.contains(&port));
        }
    }
}
