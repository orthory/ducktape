//! Settings pane ownership is checked in the authored Rust tree, not a compiler spelling.
use super::*;
use quote::ToTokens;
use syn::visit::Visit;

const SETTINGS: &str = include_str!("../../../crates/views/settings/src/lib.rs");
const PANES: [&str; 4] = ["General", "Network", "Account", "Security"];
const GROUPS: [(&str, &str); 6] = [
    ("APPEARANCE", "General"),
    ("NOTIFICATIONS", "General"),
    ("NETWORK", "Network"),
    ("YOURIDENTITY", "Account"),
    ("ACCOUNTKEYS", "Account"),
    ("IDENTITYKEY", "Security"),
];

fn pane_arms() -> Vec<(String, String)> {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(pane_arms_on_stack)
        .unwrap()
        .join()
        .unwrap()
}

fn authored_items() -> syn::File {
    let mut file = syn::parse_file(SETTINGS).unwrap();
    let items = file
        .items
        .iter()
        .flat_map(|item| match item {
            syn::Item::Macro(item)
                if item
                    .mac
                    .path
                    .segments
                    .last()
                    .unwrap()
                    .ident
                    .to_string()
                    .starts_with("__ice_generated_items_") =>
            {
                syn::parse2::<syn::File>(item.mac.tokens.clone())
                    .unwrap()
                    .items
            }
            _ => vec![item.clone()],
        })
        .collect::<Vec<_>>();
    assert!(!items.is_empty(), "authored item wrappers");
    file.items = items;
    file
}

fn pane_arms_on_stack() -> Vec<(String, String)> {
    struct Panes(Vec<(String, String)>);
    impl<'ast> Visit<'ast> for Panes {
        fn visit_arm(&mut self, arm: &'ast syn::Arm) {
            if let syn::Pat::Path(path) = &arm.pat {
                let segments: Vec<_> = path
                    .path
                    .segments
                    .iter()
                    .map(|part| part.ident.to_string())
                    .collect();
                if segments.first().is_some_and(|name| name == "SettingsPane") {
                    self.0.push((
                        segments.last().unwrap().clone(),
                        arm.body
                            .to_token_stream()
                            .to_string()
                            .chars()
                            .filter(|c| !c.is_whitespace())
                            .collect(),
                    ));
                }
            }
            syn::visit::visit_arm(self, arm);
        }
    }
    let mut visitor = Panes(Vec::new());
    let file = authored_items();
    visitor.visit_file(&file);
    struct Methods(Vec<(String, String)>);
    impl<'ast> Visit<'ast> for Methods {
        fn visit_impl_item_fn(&mut self, method: &'ast syn::ImplItemFn) {
            self.0.push((
                method.sig.ident.to_string(),
                method
                    .block
                    .to_token_stream()
                    .to_string()
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect(),
            ));
        }
    }
    let mut methods = Methods(Vec::new());
    methods.visit_file(&file);
    for (_, body) in &mut visitor.0 {
        let mut pending = vec![body.clone()];
        let mut visited = std::collections::BTreeSet::new();
        while let Some(caller) = pending.pop() {
            for (name, callee) in &methods.0 {
                if caller.contains(&format!(".{name}(")) && visited.insert(name.clone()) {
                    body.push_str(callee);
                    pending.push(callee.clone());
                }
            }
        }
    }
    visitor.0
}

#[test]
fn every_group_is_authored_under_exactly_one_pane() {
    let arms = pane_arms();
    for (group, owner) in GROUPS {
        let owners: Vec<_> = arms
            .iter()
            .filter(|(_, body)| body.contains(&format!("\"{group}\"")))
            .map(|(pane, _)| pane.as_str())
            .collect();
        assert_eq!(owners, [owner], "{group} has exactly one pane");
    }
}

#[test]
fn every_pane_has_a_tab_and_an_arm() {
    let source = rust_tokens(SETTINGS);
    let arms = pane_arms();
    assert_eq!(arms.len(), PANES.len(), "one exhaustive pane dispatch");
    for pane in PANES {
        assert!(source.contains(&format!("/settings-{}-tab", pane.to_lowercase())));
        assert_eq!(arms.iter().filter(|(name, _)| name == pane).count(), 1);
    }
}

#[test]
fn the_enum_names_the_same_panes_in_the_same_order() {
    let variants = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let file = authored_items();
            let variants = file
                .items
                .iter()
                .find_map(|item| match item {
                    syn::Item::Enum(item) if item.ident == "SettingsPane" => Some(
                        item.variants
                            .iter()
                            .map(|variant| variant.ident.to_string())
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                })
                .expect("the native pane discriminant");
            variants
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(variants, PANES);
}

#[test]
fn the_screen_branches_once_and_holds_nothing_above_the_branch() {
    let arms = pane_arms();
    let source = rust_tokens(SETTINGS);
    for (group, _) in GROUPS {
        assert_eq!(
            source.matches(&format!("\"{group}\"")).count(),
            1,
            "no heading outside its pane"
        );
        assert_eq!(
            arms.iter()
                .filter(|(_, body)| body.contains(&format!("\"{group}\"")))
                .count(),
            1
        );
    }
}

#[test]
fn the_pane_moves_only_through_the_strip() {
    let source = rust_tokens(SETTINGS);
    assert_eq!(source.matches("__local.settings_pane=").count(), 1);
    assert!(source.contains("let__ice_next=picked.clone();__local.settings_pane=__ice_next"));
    assert!(!source.contains("emit_pick_pane"));
}

#[test]
fn the_scrollable_is_the_screens_root() {
    let source = rust_tokens(SETTINGS);
    assert!(source.contains("let__ice_node_scope=format!(\"{}/settings-body\""));
    assert!(source.contains("Node::Scroll{on_scroll:"));
}
