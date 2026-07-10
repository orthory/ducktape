use std::collections::BTreeMap;

use duckdns::{
    DuckDns, DuckDnsMsg, DuckDnsName, DuckDnsQuery, DuckDnsReply, ResolvedName,
    ServiceAnnouncement, ServiceAuthority, ServiceScope, decode_reply, encode_msg, encode_query,
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
    }
}

fn account(service: &str) -> ServiceAnnouncement {
    ServiceAnnouncement {
        scope: ServiceScope::Account,
        service: service.into(),
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

    fn account_view(&self, account_id: &[u8]) -> Option<AccountView> {
        let nodes: Vec<_> = self
            .accounts
            .iter()
            .filter(|(_, account)| account.as_slice() == account_id)
            .map(|(node, _)| node.clone())
            .collect();
        if nodes.is_empty() {
            return None;
        }
        Some(AccountView {
            account_id: account_id.to_vec(),
            display_name: None,
            nonce: 0,
            member_keys: vec![],
            nodes,
            updated_at: 0,
        })
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
                    let account = self
                        .accounts
                        .get(&node_key)
                        .and_then(|account| self.account_view(account));
                    Ok(identity::encode_reply(&IdentityReply::Account(account)))
                }
                IdentityQuery::Get { account_id } => Ok(identity::encode_reply(
                    &IdentityReply::Account(self.account_view(&account_id)),
                )),
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

#[test]
fn resident_can_register_but_outsider_and_unbound_node_cannot() {
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
        DuckDnsMsg::SetHandle {
            handle: Some("orthory".into()),
        },
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();

    ctx.origin(outsider.clone());
    assert!(
        execute(
            &mut module,
            &mut ctx,
            DuckDnsMsg::SetHandle {
                handle: Some("outsider".into()),
            },
        )
        .unwrap_err()
        .to_string()
        .contains("not a validator or admitted resident")
    );

    ctx.residents.push(outsider);
    assert!(
        execute(
            &mut module,
            &mut ctx,
            DuckDnsMsg::SetHandle {
                handle: Some("unbound".into()),
            },
        )
        .unwrap_err()
        .to_string()
        .contains("not bound")
    );
}

#[test]
fn bare_account_name_resolves_identity_and_current_nodes() {
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
        DuckDnsMsg::SetHandle {
            handle: Some("orthory".into()),
        },
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();

    let registrations = block_on(module.query(&encode_query(&DuckDnsQuery::Registrations {
        from: 0,
        limit: 256,
    })))
    .expect("registration query");
    let DuckDnsReply::Registrations(registrations) = decode_reply(&registrations).unwrap() else {
        panic!("registration query returned the wrong variant");
    };
    assert_eq!(registrations.len(), 1);
    assert_eq!(registrations[0].handle, "orthory");
    assert_eq!(registrations[0].account_id, b"owner");

    let DuckDnsReply::Resolved(Some(ResolvedName::Account(resolved))) = resolve(
        &module,
        &ctx,
        DuckDnsName::Account {
            handle: "orthory".into(),
        },
    ) else {
        panic!("account name did not resolve");
    };
    assert_eq!(resolved.account_id, b"owner");
    assert_eq!(resolved.nodes.len(), 2);

    ctx.validators.retain(|candidate| candidate != &second);
    let DuckDnsReply::Resolved(Some(ResolvedName::Account(filtered))) = resolve(
        &module,
        &ctx,
        DuckDnsName::Account {
            handle: "orthory".into(),
        },
    ) else {
        panic!("account name did not resolve after standing changed");
    };
    assert_eq!(filtered.nodes.len(), 1);
    assert_eq!(filtered.nodes[0].node, first);
}

#[test]
fn account_service_authority_is_derived_from_origin_and_filters_rebinding() {
    let owner_node = node(1);
    let other_node = node(2);
    let mut ctx = TestCtx::new(owner_node.clone());
    ctx.validators = vec![owner_node.clone(), other_node.clone()];
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
        DuckDnsMsg::SetHandle {
            handle: Some("orthory".into()),
        },
    )
    .unwrap();

    ctx.origin(other_node.clone());
    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::ReplaceAnnouncements {
            announcements: vec![account("huddle")],
        },
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();
    assert_eq!(
        resolve(
            &module,
            &ctx,
            DuckDnsName::AccountService {
                service: "huddle".into(),
                handle: "orthory".into(),
            },
        ),
        DuckDnsReply::Resolved(None),
        "another account's declaration is not projected beneath this alias"
    );

    ctx.origin(owner_node.clone());
    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::ReplaceAnnouncements {
            announcements: vec![account("huddle")],
        },
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();

    let DuckDnsReply::Resolved(Some(ResolvedName::Service(resolved))) = resolve(
        &module,
        &ctx,
        DuckDnsName::AccountService {
            service: "huddle".into(),
            handle: "orthory".into(),
        },
    ) else {
        panic!("account service did not resolve");
    };
    assert_eq!(resolved.providers.len(), 1);
    assert_eq!(
        resolved.authority,
        ServiceAuthority::Account {
            account_id: b"owner".to_vec(),
        }
    );
    assert_eq!(resolved.providers[0].node, owner_node);

    ctx.bind(node(1), b"other");
    assert_eq!(
        resolve(
            &module,
            &ctx,
            DuckDnsName::AccountService {
                service: "huddle".into(),
                handle: "orthory".into(),
            },
        ),
        DuckDnsReply::Resolved(None)
    );

    let registration = block_on(module.query_with(
        &ctx,
        &encode_query(&DuckDnsQuery::NodeRegistration { node: node(1) }),
    ))
    .unwrap();
    assert_eq!(
        decode_reply(&registration).unwrap(),
        DuckDnsReply::NodeRegistration {
            registration: Some(duckdns::NodeRegistration {
                account_id: Some(b"owner".to_vec()),
                announcements: vec![account("huddle")],
            }),
            authority_current: false,
        },
        "the announcer must observe and replace a stale account binding"
    );
}

#[test]
fn network_and_node_scoped_discovery_recheck_standing() {
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
            announcements: vec![network("search")],
        },
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();

    let DuckDnsReply::Resolved(Some(ResolvedName::Service(resolved))) = resolve(
        &module,
        &ctx,
        DuckDnsName::NodeService {
            service: "search".into(),
            node: "n-010101010101".into(),
            chain: "team-a1b2c3d4".into(),
        },
    ) else {
        panic!("node-scoped service did not resolve");
    };
    assert_eq!(resolved.providers.len(), 1);

    ctx.residents.clear();
    assert_eq!(
        resolve(
            &module,
            &ctx,
            DuckDnsName::NetworkService {
                service: "search".into(),
                chain: "team-a1b2c3d4".into(),
            },
        ),
        DuckDnsReply::Resolved(None)
    );
}
