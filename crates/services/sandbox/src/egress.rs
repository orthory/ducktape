//! the egress firewall for a run's tap device.
//!
//! Under podman this was an OCI `createRuntime` hook that entered the
//! container's netns with `nsenter`. A microVM has no netns to enter: its
//! network device is a tap that exists in the HOST's namespace, so the ruleset
//! is loaded here, by the host, filtering on the interface name. No hook, no
//! nsenter, no annotations.
//!
//! ORDER IS LOAD-BEARING and is what the tests pin. The broker and DNS accepts
//! must precede the private-range drop, because the addresses they name sit
//! *inside* the ranges being dropped — reverse the two and the run cannot reach
//! the broker it needs, which presents as a hang rather than a refusal.
//!
//! A tap requires `CAP_NET_ADMIN`, so a run only gets one on a node whose
//! operator provisioned it. A node without that capability runs its guests with
//! no network device at all and reaches the broker over vsock — see
//! [`crate::microvm`]. That is a stricter configuration, not a degraded one.

/// the nftables ruleset for one run's tap. Pure, so the ordering below is
/// unit-tested without root and without a VMM.
///
/// `host_ip` is the host end of the tap's point-to-point link — where the
/// broker and the node's RPC answer. `guest_ip` is the VM's own address, used
/// for the NAT source match.
///
/// DNS is scoped to `host_ip` ONLY, never a blanket `dport 53`. The lesson is
/// inherited: on a tailnet box a blanket rule also opens the MagicDNS resolvers
/// (100.100.100.100 / fd7a::53) and any LAN box on :53, which are exactly the
/// hosts the private-range drop below exists to deny.
pub fn tap_egress_nftables(
    tap: &str,
    host_ip: &str,
    guest_ip: &str,
    broker_ports: &[u16],
) -> String {
    let table = format!("ducktape_{}", tap.replace('-', "_"));
    let mut lines = vec![
        format!("table inet {table} {{"),
        "  chain input {".to_string(),
        "    type filter hook input priority 0; policy accept;".to_string(),
    ];

    // ---- what the guest may reach ON THIS HOST -----------------------------
    if !broker_ports.is_empty() {
        let allowed = broker_ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!(
            "    iifname \"{tap}\" ip daddr {host_ip} tcp dport {{ {allowed} }} accept"
        ));
    }
    lines.push(format!(
        "    iifname \"{tap}\" ip daddr {host_ip} udp dport 53 accept"
    ));
    lines.push(format!(
        "    iifname \"{tap}\" ip daddr {host_ip} tcp dport 53 accept"
    ));
    // everything else aimed at this host is refused: the guest talks to the
    // broker and the resolver, not to whatever else the operator runs locally.
    lines.push(format!("    iifname \"{tap}\" drop"));
    lines.push("  }".to_string());

    // ---- where the guest may be routed -------------------------------------
    lines.push("  chain forward {".to_string());
    lines.push("    type filter hook forward priority 0; policy accept;".to_string());
    lines.push(format!(
        "    iifname \"{tap}\" ip daddr {{ 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, \
         100.64.0.0/10, 169.254.0.0/16, 127.0.0.0/8 }} drop"
    ));
    // IPv6: ULA (which covers tailnet fd7a::/48), link-local and loopback.
    // Public v6 falls through to policy accept, same as v4.
    lines.push(format!(
        "    iifname \"{tap}\" ip6 daddr {{ fc00::/7, fe80::/10, ::1/128 }} drop"
    ));
    lines.push("  }".to_string());
    lines.push("}".to_string());

    // ---- NAT so the public internet is actually reachable ------------------
    lines.push(format!("table ip {table}_nat {{"));
    lines.push("  chain postrouting {".to_string());
    lines.push("    type nat hook postrouting priority 100; policy accept;".to_string());
    lines.push(format!("    ip saddr {guest_ip} masquerade"));
    lines.push("  }".to_string());
    lines.push("}".to_string());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ruleset() -> String {
        tap_egress_nftables("dtap7", "169.254.7.1", "169.254.7.2", &[8080, 9090])
    }

    /// The accepts name addresses that live inside the dropped ranges. If the
    /// drop came first the run could not reach its own broker — and it would
    /// present as a hang, not a refusal.
    #[test]
    fn the_broker_accept_precedes_the_private_range_drop() {
        let rules = ruleset();
        let accept = rules
            .find("tcp dport { 8080, 9090 } accept")
            .expect("accept");
        let drop = rules.find("10.0.0.0/8").expect("private drop");
        assert!(accept < drop, "accept must precede drop:\n{rules}");
    }

    /// A blanket `dport 53` would open the tailnet's MagicDNS resolvers and any
    /// LAN box on :53 — the exact hosts the private-range drop denies.
    #[test]
    fn dns_is_scoped_to_the_host_never_opened_universally() {
        let rules = ruleset();
        for line in rules.lines().filter(|l| l.contains("dport 53")) {
            assert!(
                line.contains("ip daddr 169.254.7.1"),
                "unscoped DNS rule: {line}"
            );
        }
    }

    /// The guest may reach the broker and the resolver on this host. Everything
    /// else the operator happens to run locally is not part of the deal.
    #[test]
    fn nothing_else_on_the_host_is_reachable() {
        let rules = ruleset();
        let input_chain: Vec<&str> = rules
            .lines()
            .skip_while(|l| !l.contains("chain input"))
            .take_while(|l| !l.contains("chain forward"))
            .collect();
        assert_eq!(
            input_chain.last().map(|l| l.trim()),
            Some("}"),
            "{input_chain:?}"
        );
        assert!(
            input_chain
                .iter()
                .any(|l| l.trim() == "iifname \"dtap7\" drop"),
            "the input chain must end in a drop:\n{input_chain:?}"
        );
    }

    /// Both families, or the drop is decorative: a tailnet reachable over IPv6
    /// is just as reachable.
    #[test]
    fn both_address_families_are_dropped() {
        let rules = ruleset();
        assert!(rules.contains("100.64.0.0/10"), "v4 CGNAT/tailnet");
        assert!(rules.contains("fc00::/7"), "v6 ULA");
    }

    /// A run with no broker ports must not emit an empty set — `tcp dport { }`
    /// is a syntax error and nft would reject the whole ruleset, taking the
    /// private-range drops down with it.
    #[test]
    fn a_run_with_no_broker_ports_still_produces_a_valid_ruleset() {
        let rules = tap_egress_nftables("dtap0", "169.254.0.1", "169.254.0.2", &[]);
        assert!(!rules.contains("dport { }"), "{rules}");
        assert!(rules.contains("10.0.0.0/8"), "the drops must survive");
    }

    /// Two concurrent runs each load their own table; a shared name would have
    /// the second run's load replace the first run's rules.
    #[test]
    fn each_tap_gets_its_own_table() {
        let a = tap_egress_nftables("dtap1", "169.254.1.1", "169.254.1.2", &[]);
        let b = tap_egress_nftables("dtap2", "169.254.2.1", "169.254.2.2", &[]);
        assert!(a.contains("table inet ducktape_dtap1"), "{a}");
        assert!(b.contains("table inet ducktape_dtap2"), "{b}");
    }
}
