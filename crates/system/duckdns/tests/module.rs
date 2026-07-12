use std::collections::BTreeMap;

use duckdns::{
    DuckDns, DuckDnsMsg, DuckDnsName, DuckDnsQuery, DuckDnsReply, ResolvedAccount, decode_reply,
    encode_msg, encode_query,
};
use futures::executor::block_on;
use identity::{AccountView, IdentityQuery, IdentityReply, decode_query as identity_decode_query};
use sdk::{Ctx, Effect, Env, Error, Event, Module, Msg, Origin, StateRoot};
use valset::{ValsetQuery, ValsetReply, decode_query as valset_decode_query};

fn node(byte: u8) -> Vec<u8> {
    vec![byte; 32]
}

fn name(handle: &str) -> DuckDnsName {
    DuckDnsName {
        handle: handle.into(),
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

fn execute(module: &mut DuckDns, ctx: &mut TestCtx, message: DuckDnsMsg) -> Result<(), Error> {
    block_on(module.execute(
        ctx,
        &Msg {
            target: "duckdns".into(),
            payload: encode_msg(&message),
        },
    ))
}

fn resolve(module: &DuckDns, handle: &str) -> DuckDnsReply {
    let bytes =
        block_on(module.query(&encode_query(&DuckDnsQuery::Resolve { name: name(handle) })))
            .unwrap();
    decode_reply(&bytes).unwrap()
}

#[test]
fn resident_can_register_but_outsider_and_unbound_node_cannot() {
    let resident = node(1);
    let outsider = node(2);
    let mut ctx = TestCtx::new(resident.clone());
    ctx.residents.push(resident.clone());
    ctx.bind(resident.clone(), b"account-a");
    let mut module = DuckDns::new("duckdns", "identity", Some("valset".into()));

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

/// a SetHandle for a reserved root label is refused at ADMISSION — the op never
/// commits, so `agents.duck` can never be owned by an account. (What a snapshot
/// already holding one does is `registry.rs`'s
/// `a_snapshot_holding_a_newly_reserved_handle_still_installs_and_is_inert`: it
/// decodes, and stays inert.)
#[test]
fn a_reserved_root_label_never_admits() {
    let resident = node(1);
    let mut ctx = TestCtx::new(resident.clone());
    ctx.residents.push(resident.clone());
    ctx.bind(resident, b"account-a");
    let mut module = DuckDns::new("duckdns", "identity", Some("valset".into()));

    for label in duckdns::RESERVED_ROOT_LABELS {
        let err = execute(
            &mut module,
            &mut ctx,
            DuckDnsMsg::SetHandle {
                handle: Some((*label).into()),
            },
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("reserved"), "{label}: {err}");
    }
    block_on(module.commit_block()).unwrap();
    assert_eq!(resolve(&module, "orthory"), DuckDnsReply::Resolved(None));
}

#[test]
fn account_name_resolves_only_account_id_and_unregisters_cleanly() {
    let first = node(1);
    let second = node(2);
    let mut ctx = TestCtx::new(first.clone());
    ctx.validators = vec![first.clone(), second.clone()];
    ctx.bind(first, b"owner");
    ctx.bind(second, b"owner");
    let mut module = DuckDns::new("duckdns", "identity", Some("valset".into()));

    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::SetHandle {
            handle: Some("orthory".into()),
        },
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();

    assert_eq!(
        resolve(&module, "orthory"),
        DuckDnsReply::Resolved(Some(ResolvedAccount {
            account_id: b"owner".to_vec(),
        }))
    );

    let registrations = block_on(module.query(&encode_query(&DuckDnsQuery::Registrations {
        from: 0,
        limit: 256,
    })))
    .unwrap();
    let DuckDnsReply::Registrations(registrations) = decode_reply(&registrations).unwrap() else {
        panic!("registration query returned the wrong variant");
    };
    assert_eq!(registrations.len(), 1);
    assert_eq!(registrations[0].handle, "orthory");
    assert_eq!(registrations[0].account_id, b"owner");

    execute(
        &mut module,
        &mut ctx,
        DuckDnsMsg::SetHandle { handle: None },
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();
    assert_eq!(resolve(&module, "orthory"), DuckDnsReply::Resolved(None));
}
