//! Settings pane ownership is checked in the authored Rust tree, not a compiler spelling.
use super::*;
use quote::ToTokens;
use syn::visit::Visit;

const SETTINGS: &str = include_str!("../../../crates/views/settings/src/lib.rs");
const PANES: [&str; 4] = ["General", "Network", "Account", "Security"];
const GROUPS: [(&str, &str); 6] = [
    ("settings/appearance", "General"),
    ("settings/notifications", "General"),
    ("settings/network", "Network"),
    ("settings/identity-title", "Account"),
    ("settings/keys-title", "Account"),
    ("settings/security-title", "Security"),
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
    syn::parse_file(SETTINGS).unwrap()
}

fn view_method(file: &syn::File) -> &syn::ImplItemFn {
    file.items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(item) => Some(&item.items),
            _ => None,
        })
        .flatten()
        .find_map(|item| match item {
            syn::ImplItem::Fn(method) if method.sig.ident == "view" => Some(method),
            _ => None,
        })
        .expect("Settings has one authored view")
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
    visitor.visit_impl_item_fn(view_method(&file));
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
    let file = authored_items();
    let method = view_method(&file);
    let tabs = method
        .block
        .stmts
        .iter()
        .find_map(|statement| {
            let syn::Stmt::Local(local) = statement else {
                return None;
            };
            let syn::Pat::Ident(binding) = &local.pat else {
                return None;
            };
            if binding.ident != "tabs" {
                return None;
            }
            let syn::Expr::Array(array) = local.init.as_ref()?.expr.as_ref() else {
                return None;
            };
            Some(array)
        })
        .expect("the tab strip declares its pane choices");
    let arms = pane_arms();
    assert_eq!(arms.len(), PANES.len(), "one exhaustive pane dispatch");
    assert_eq!(tabs.elems.len(), PANES.len(), "one tab per pane");
    for (choice, pane) in tabs.elems.iter().zip(PANES) {
        let syn::Expr::Tuple(choice) = choice else {
            panic!("tab choice is key, label, pane");
        };
        assert_eq!(choice.elems.len(), 3);
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(key),
            ..
        }) = &choice.elems[0]
        else {
            panic!("tab key");
        };
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(label),
            ..
        }) = &choice.elems[1]
        else {
            panic!("tab label");
        };
        let syn::Expr::Path(target) = &choice.elems[2] else {
            panic!("tab pane route");
        };
        assert_eq!(key.value(), pane.to_lowercase());
        assert_eq!(label.value(), pane);
        assert_eq!(
            target.path.to_token_stream().to_string(),
            format!("SettingsPane :: {pane}")
        );
        assert_eq!(arms.iter().filter(|(name, _)| name == pane).count(), 1);
    }
    let body = rust_tokens(&method.block.to_token_stream().to_string());
    assert!(body.contains("tabs.into_iter().map(|(key,label,pane)|"));
    assert!(body.contains("Message::PickSettingsPane(SETTINGS_SCOPE.into(),pane)"));
    assert!(
        body.contains("Some(state.settings_pane==pane)"),
        "native checked state follows the same pane"
    );
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
    let file = authored_items();
    let body = rust_tokens(&view_method(&file).block.to_token_stream().to_string());
    assert_eq!(body.matches("matchstate.settings_pane").count(), 1);
    for (group, _) in GROUPS {
        assert!(
            !body.contains(&format!("\"{group}\"")),
            "groups belong to pane methods, never the common screen"
        );
    }
}

#[test]
fn the_pane_moves_only_through_the_strip() {
    let source = rust_tokens(SETTINGS);
    assert_eq!(source.matches("local.settings_pane=").count(), 1);
    assert!(source.contains("local.settings_pane=picked.clone()"));
    assert!(!source.contains("emit_pick_pane"));
}

#[test]
fn the_scrollable_is_the_screens_root() {
    let file = authored_items();
    let method = view_method(&file);
    let Some(syn::Stmt::Expr(syn::Expr::Call(root), None)) = method.block.stmts.last() else {
        panic!("the view returns its scroll root directly");
    };
    let syn::Expr::Path(function) = root.func.as_ref() else {
        panic!("named root builder");
    };
    assert_eq!(function.path.to_token_stream().to_string(), "kit :: scroll");
    assert_eq!(root.args.len(), 2);
    assert!(
        rust_tokens(&root.args[1].to_token_stream().to_string())
            .contains("kit::column(\"settings/content\",content)"),
        "the entire header, tab strip and selected pane share the scroll root"
    );
}
