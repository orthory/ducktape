//! The native machine over the frozen suite: every scenario, against the
//! shared fixture set.

netstack_scenarios::suite!(netstack_scenarios::native);

/// Every scenario the crate defines is one the `suite!` macro runs — a
/// scenario left out of the macro would be a fixture nobody checks.
#[test]
fn every_scenario_is_in_the_suite() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenarios = std::fs::read_to_string(root.join("src/scenarios.rs")).unwrap();
    let suite = std::fs::read_to_string(root.join("src/lib.rs")).unwrap();
    let defined: Vec<&str> = scenarios
        .lines()
        .filter_map(|line| line.strip_prefix("pub fn "))
        .filter_map(|rest| rest.split_once("(backend: Backend)"))
        .map(|(name, _)| name)
        .collect();
    let listed: Vec<&str> = suite
        .split_once("suite!(@each $backend;")
        .and_then(|(_, rest)| rest.split_once(");"))
        .map(|(names, _)| {
            names
                .split(',')
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .collect()
        })
        .unwrap_or_default();
    assert!(!defined.is_empty());
    for name in &defined {
        assert!(
            listed.contains(name),
            "scenario `{name}` is not in the suite! macro"
        );
    }
    for name in &listed {
        assert!(
            defined.contains(name),
            "suite! lists `{name}` but no scenario defines it"
        );
    }
}
