use std::collections::BTreeMap;

use duckdns::{
    DuckDns, DuckDnsMsg, DuckDnsName, DuckDnsQuery, DuckDnsReply, ServiceAnnouncement,
    ServiceScope, decode_reply, encode_msg, encode_query,
};
use futures::executor::block_on;
use identity::{AccountView, IdentityQuery, IdentityReply, decode_query as identity_decode_query};
use sdk::{Ctx, Effect, Env, Error, Event, Module, Msg, Origin, StateRoot};
use valset::{ValsetQuery, ValsetReply, decode_query as valset_decode_query};

fn node(byte: u8) -> Vec<u8> {
    vec![byte; 32]
}

fn network(service: &str) -> ServiceAnnouncement {
    ServiceAnnouncement {
        scope: ServiceScope::Network,
        service: service.into(),
        default_homepage: false,
        allow_cross_site: false,
    }
}

fn user(handle: &str, service: &str, default_homepage: bool) -> ServiceAnnouncement {
    ServiceAnnouncement {
        scope: ServiceScope::User {
            handle: handle.into(),
        },
        service: service.into(),
        default_homepage,
        allow_cross_site: false,
    }
}

struct TestCtx {
    env: Env,
    validators: Vec<Vec<u8>>,
    residents: Vec<Vec<u8>>,
    accounts: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl TestCtx {
    fn new(origin: Vec<u8>) -> Self {
        Self {
            env: Env {
                height: 1,
                consensus_time: 1,
                origin: Origin::External(origin),
                me: "duckdns".into(),
                protocol_version: 0,
            },
            validators: Vec::new(),
            residents: Vec::new(),
            accounts: BTreeMap::new(),
        }
    }

    fn origin(&mut self, node: Vec<u8>) {
        self.env.origin = Origin::External(node);
    }

    fn bind(&mut self, node: Vec<u8>, account: &[u8]) {
        self.accounts.insert(node, account.to_vec());
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
        &self.env
    }

    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }

    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        match target {
            "valset" => match valset_decode_query(req).map_err(Error::Module)? {
                ValsetQuery::Validators => Ok(valset::encode_reply(&ValsetReply::Validators(
                    self.validators.clone(),
                ))),
                ValsetQuery::Residents => Ok(valset::encode_reply(&ValsetReply::Residents(
                    self.residents.clone(),
                ))),
            },
            "identity" => match identity_decode_query(req).map_err(Error::Module)? {
                IdentityQuery::OfNode { node_key } => {
                    let account = self.accounts.get(&node_key).map(|account_id| AccountView {
                        account_id: account_id.clone(),
                        display_name: None,
                        nonce: 0,
                        member_keys: vec![],
                        nodes: vec![node_key],
                        updated_at: 0,
                    });
                    Ok(identity::encode_reply(&IdentityReply::Account(account)))
                }
                _ => Err(Error::QueryUnsupported),
            },
            _ => Err(Error::UnknownModule(target.into())),
        }
    }

    fn emit_msg(&mut self, _msg: Msg) {}
    fn emit_event(&mut self, _event: Event) {}
    fn request_effect(&mut self, _effect: Effect) {}
}

fn message(message: DuckDnsMsg) -> Msg {
    Msg {
        target: "duckdns".into(),
        payload: encode_msg(&message),
    }
}

fn execute(module: &mut DuckDns, ctx: &mut TestCtx, message: DuckDnsMsg) -> Result<(), Error> {
    block_on(module.execute(ctx, &self::message(message)))
}

fn resolve(module: &DuckDns, ctx: &TestCtx, name: DuckDnsName) -> DuckDnsReply {
    let bytes =
        block_on(module.query_with(ctx, &encode_query(&DuckDnsQuery::Resolve { name }))).unwrap();
    decode_reply(&bytes).unwrap()
}

fn namespace(module: &DuckDns, ctx: &TestCtx) -> Vec<String> {
    let bytes =
        block_on(module.query_with(ctx, &encode_query(&DuckDnsQuery::Namespace))).unwrap();
    let DuckDnsReply::Namespace(names) = decode_reply(&bytes).unwrap() else {
        panic!("namespace query returned another reply shape");
    };
    names
}

#[test]
fn resident_can_claim_but_outsider_and_unbound_node_cannot() {
    let resident = node(1);
    let outsider = node(2);
    let mut ctx = TestCtx::new(resident.clone());
    ctx.residents.push(resident.clone());
    ctx.bind(resident.clone(), b"account-a");
    let mut module = DuckDns::new(
        "duckdns",
        "identity",
        Some("valset".into()),
        "team#a1b2c3d4",
    )
    .unwrap();

    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::ClaimHandle {
            handle: "orthory".into(),
        },
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();

    ctx.origin(outsider.clone());
    assert!(
        execute(
            &mut module,
            &mut ctx,
            DuckDnsMsg::ClaimHandle {
                handle: "outsider".into(),
            },
        )
        .unwrap_err()
        .to_string()
        .contains("not a validator or admitted resident")
    );

    ctx.residents.push(outsider.clone());
    assert!(
        execute(
            &mut module,
            &mut ctx,
            DuckDnsMsg::ClaimHandle {
                handle: "unbound".into(),
            },
        )
        .unwrap_err()
        .to_string()
        .contains("not bound")
    );
}

#[test]
fn user_scope_uses_identity_owner_while_network_scope_needs_only_standing() {
    let owner_node = node(1);
    let other_node = node(2);
    let network_only = node(3);
    let mut ctx = TestCtx::new(owner_node.clone());
    ctx.validators = vec![owner_node.clone(), other_node.clone(), network_only.clone()];
    ctx.bind(owner_node.clone(), b"owner");
    ctx.bind(other_node.clone(), b"other");
    let mut module = DuckDns::new(
        "duckdns",
        "identity",
        Some("valset".into()),
        "team#a1b2c3d4",
    )
    .unwrap();

    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::ClaimHandle {
            handle: "orthory".into(),
        },
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();

    ctx.origin(other_node.clone());
    assert!(
        execute(
            &mut module,
            &mut ctx,
            DuckDnsMsg::ReplaceAnnouncements {
                announcements: vec![user("orthory", "home", true)],
            },
        )
        .unwrap_err()
        .to_string()
        .contains("does not own")
    );

    ctx.origin(network_only);
    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::ReplaceAnnouncements {
            announcements: vec![network("docs")],
        },
    )
    .unwrap();
}

#[test]
fn resolution_rechecks_standing_and_identity_binding() {
    let first = node(1);
    let second = node(2);
    let mut ctx = TestCtx::new(first.clone());
    ctx.validators = vec![first.clone(), second.clone()];
    ctx.bind(first.clone(), b"owner");
    ctx.bind(second.clone(), b"owner");
    let mut module = DuckDns::new(
        "duckdns",
        "identity",
        Some("valset".into()),
        "team#a1b2c3d4",
    )
    .unwrap();

    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::ClaimHandle {
            handle: "orthory".into(),
        },
    )
    .unwrap();
    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::ReplaceAnnouncements {
            announcements: vec![network("docs"), user("orthory", "home", true)],
        },
    )
    .unwrap();
    ctx.origin(second.clone());
    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::ReplaceAnnouncements {
            announcements: vec![network("docs"), user("orthory", "home", true)],
        },
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();

    let DuckDnsReply::Resolved(Some(network_resolution)) = resolve(
        &module,
        &ctx,
        DuckDnsName::NetworkService {
            service: "docs".into(),
            chain: "team-a1b2c3d4".into(),
        },
    ) else {
        panic!("network service did not resolve");
    };
    assert_eq!(network_resolution.providers.len(), 2);

    ctx.validators = vec![first.clone()];
    let DuckDnsReply::Resolved(Some(filtered)) = resolve(
        &module,
        &ctx,
        DuckDnsName::NetworkService {
            service: "docs".into(),
            chain: "team-a1b2c3d4".into(),
        },
    ) else {
        panic!("standing provider did not resolve");
    };
    assert_eq!(filtered.providers.len(), 1);
    assert_eq!(filtered.providers[0].node, first);

    ctx.accounts.remove(&first);
    assert_eq!(
        resolve(
            &module,
            &ctx,
            DuckDnsName::User {
                handle: "orthory".into(),
            },
        ),
        DuckDnsReply::Resolved(None)
    );
}

#[test]
fn namespace_snapshot_rechecks_live_standing_and_user_binding() {
    let provider = node(1);
    let stale = node(2);
    let mut ctx = TestCtx::new(provider.clone());
    ctx.validators = vec![provider.clone(), stale.clone()];
    ctx.bind(provider.clone(), b"owner");
    ctx.bind(stale.clone(), b"owner");
    let mut module = DuckDns::new(
        "duckdns",
        "identity",
        Some("valset".into()),
        "team#a1b2c3d4",
    )
    .unwrap();
    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::ClaimHandle {
            handle: "orthory".into(),
        },
    )
    .unwrap();
    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::ReplaceAnnouncements {
            announcements: vec![network("docs"), user("orthory", "home", true)],
        },
    )
    .unwrap();
    ctx.origin(stale.clone());
    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::ReplaceAnnouncements {
            announcements: vec![network("status")],
        },
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();

    let names = namespace(&module, &ctx);
    assert!(names.contains(&"orthory.ducktape.quack".into()));
    assert!(names.contains(&"docs.team-a1b2c3d4.net.ducktape.quack".into()));
    assert!(names.contains(&"status.team-a1b2c3d4.net.ducktape.quack".into()));

    ctx.validators.retain(|node| node != &stale);
    let names = namespace(&module, &ctx);
    assert!(!names.iter().any(|name| name.starts_with("status.")));
    ctx.accounts.remove(&provider);
    let names = namespace(&module, &ctx);
    assert!(!names.iter().any(|name| name.contains("orthory")));
    assert!(names.contains(&"docs.team-a1b2c3d4.net.ducktape.quack".into()));
}

#[test]
fn node_qualified_name_never_fails_over() {
    let provider = node(1);
    let mut ctx = TestCtx::new(provider.clone());
    ctx.residents = vec![provider.clone()];
    let mut module = DuckDns::new(
        "duckdns",
        "identity",
        Some("valset".into()),
        "team#a1b2c3d4",
    )
    .unwrap();
    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::ReplaceAnnouncements {
            announcements: vec![network("docs")],
        },
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();

    ctx.residents.clear();
    assert_eq!(
        resolve(
            &module,
            &ctx,
            DuckDnsName::NodeService {
                service: "docs".into(),
                node: "n-010101010101".into(),
                chain: "team-a1b2c3d4".into(),
            },
        ),
        DuckDnsReply::Resolved(None)
    );
}
