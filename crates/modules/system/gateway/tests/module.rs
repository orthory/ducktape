use std::collections::{BTreeMap, BTreeSet};

use commonware_cryptography::{Signer as _, ed25519};
use futures::executor::block_on;
use gateway::{
    CredentialGrantStatement, CredentialKind, CredentialRecord, DuckDnsName, GATEWAY_CREDENTIAL_NS,
    GATEWAY_ROUTE_NS, Gateway, GatewayMsg, GatewayQuery, GatewayReply, MemberAuthorization,
    RemoveCredentialStatement, ResolvedAccount, RouteAudience, RouteDefinition, RouteMethod,
    RouteName, RoutePolicy, RouteStatement, RouteTarget, SetCredentialStatement,
    credential_use_allowed, decode_msg, decode_reply, encode_msg, encode_query,
    grant_credential_preimage, remove_credential_preimage, revoke_credential_preimage,
    route_signing_preimage, set_credential_preimage, validate_credential_name,
};
use identity::{AccountView, IdentityQuery, IdentityReply, KeyKind, MemberKeyView, NodeView};
use sdk::{Ctx, Env, Error, Event, Module, Msg, Origin, StateRoot};
use valset::{ValsetQuery, ValsetReply};

struct TestCtx {
    env: Env,
    validators: Vec<Vec<u8>>,
    residents: Vec<Vec<u8>>,
    accounts: BTreeMap<Vec<u8>, AccountView>,
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
        &self.env
    }

    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }

    async fn query(&self, target: &str, request: &[u8]) -> Result<Vec<u8>, Error> {
        match target {
            "identity" => match identity::decode_query(request).map_err(Error::Module)? {
                IdentityQuery::OfNode { node_key } => Ok(identity::encode_reply(
                    &IdentityReply::Account(self.accounts.get(&node_key).cloned()),
                )),
                _ => Err(Error::QueryUnsupported),
            },
            "valset" => match valset::decode_query(request).map_err(Error::Module)? {
                ValsetQuery::Validators => Ok(valset::encode_reply(&ValsetReply::Validators(
                    self.validators.clone(),
                ))),
                ValsetQuery::Residents => Ok(valset::encode_reply(&ValsetReply::Residents(
                    self.residents.clone(),
                ))),
                ValsetQuery::MeshWindow => {
                    Ok(valset::encode_reply(&ValsetReply::MeshWindow(Vec::new())))
                }
            },
            _ => Err(Error::UnknownModule(target.into())),
        }
    }

    fn emit_msg(&mut self, _msg: Msg) {}
    fn emit_event(&mut self, _event: Event) {}
}

fn account(node: &[u8], signer: &ed25519::PrivateKey) -> AccountView {
    AccountView {
        account_id: signer.public_key().as_ref().to_vec(),
        display_name: Some("Alice".into()),
        avatar: None,
        bio: None,
        nonce: 0,
        member_keys: vec![MemberKeyView {
            pubkey: signer.public_key().as_ref().to_vec(),
            kind: KeyKind::Ed25519,
            label: None,
            added_at: 0,
        }],
        nodes: vec![NodeView {
            node_key: node.to_vec(),
            label: None,
        }],
        updated_at: 0,
    }
}

fn statement(account_id: Vec<u8>, node: Vec<u8>, name: RouteName, revision: u64) -> RouteStatement {
    RouteStatement {
        chain_id: "test#12345678".into(),
        account_id,
        name,
        publisher_node: node,
        revision,
        route: Some(RouteDefinition {
            target: RouteTarget::LoopbackHttp,
            policy: RoutePolicy {
                audience: RouteAudience::Network,
                methods: vec![RouteMethod::Get, RouteMethod::Post],
                max_request_bytes: 4096,
                max_response_bytes: 8192,
                allow_authorization: false,
                allow_upgrade: false,
            },
        }),
    }
}

fn signed(statement: RouteStatement, signer: &ed25519::PrivateKey) -> GatewayMsg {
    let signature = signer
        .sign(
            GATEWAY_ROUTE_NS,
            &route_signing_preimage(&statement).unwrap(),
        )
        .as_ref()
        .to_vec();
    GatewayMsg::SetRoute {
        statement,
        authorization: MemberAuthorization {
            signer: signer.public_key().as_ref().to_vec(),
            signature,
        },
    }
}

fn execute(module: &mut Gateway, ctx: &mut TestCtx, message: GatewayMsg) -> Result<(), Error> {
    block_on(module.execute(
        ctx,
        &Msg {
            target: "gateway".into(),
            payload: encode_msg(&message),
        },
    ))
}

fn fixture(seed: u64) -> (Vec<u8>, ed25519::PrivateKey, AccountView, TestCtx, Gateway) {
    let node = vec![1; 32];
    let signer = ed25519::PrivateKey::from_seed(seed);
    let account = account(&node, &signer);
    let context = TestCtx {
        env: Env {
            height: 1,
            consensus_time: 1,
            origin: Origin::External(node.clone()),
            me: "gateway".into(),
        },
        validators: vec![node.clone()],
        residents: vec![],
        accounts: BTreeMap::from([(node.clone(), account.clone())]),
    };
    let module = Gateway::new(
        "gateway",
        Box::new(sdk_testkit::MemStore::new()),
        "identity",
        Some("valset".into()),
        "test#12345678",
    );
    (node, signer, account, context, module)
}

#[test]
fn route_requires_standing_bound_origin_current_member_and_valid_signature() {
    let (node, signer, account, mut context, mut module) = fixture(7);
    let outsider = vec![2; 32];
    let publish = signed(
        statement(
            account.account_id.clone(),
            node.clone(),
            RouteName::named("api"),
            1,
        ),
        &signer,
    );
    execute(&mut module, &mut context, publish.clone()).unwrap();
    block_on(module.commit_block()).unwrap();

    let reply = block_on(module.query(&encode_query(&GatewayQuery::Get {
        account_id: account.account_id.clone(),
        name: RouteName::named("api"),
    })))
    .unwrap();
    assert!(matches!(
        decode_reply(&reply).unwrap(),
        GatewayReply::Route(route) if route.is_some()
    ));

    context.env.origin = Origin::External(outsider.clone());
    assert!(
        execute(&mut module, &mut context, publish.clone())
            .unwrap_err()
            .to_string()
            .contains("validator or admitted resident")
    );

    context.validators.push(outsider);
    assert!(
        execute(&mut module, &mut context, publish.clone())
            .unwrap_err()
            .to_string()
            .contains("publisher")
    );

    context.env.origin = Origin::External(node);
    let mut forged = publish;
    let GatewayMsg::SetRoute { authorization, .. } = &mut forged else {
        unreachable!("publish is a SetRoute");
    };
    authorization.signature[0] ^= 1;
    assert!(
        execute(&mut module, &mut context, forged)
            .unwrap_err()
            .to_string()
            .contains("verify")
    );
}

#[test]
fn apex_and_named_routes_share_one_authority_model_but_independent_revisions() {
    let (node, signer, account, mut context, mut module) = fixture(8);
    for name in [RouteName::apex(), RouteName::named("api")] {
        execute(
            &mut module,
            &mut context,
            signed(
                statement(account.account_id.clone(), node.clone(), name, 1),
                &signer,
            ),
        )
        .unwrap();
    }
    block_on(module.commit_block()).unwrap();

    for name in [RouteName::apex(), RouteName::named("api")] {
        let reply = block_on(module.query(&encode_query(&GatewayQuery::Get {
            account_id: account.account_id.clone(),
            name,
        })))
        .unwrap();
        assert!(matches!(
            decode_reply(&reply).unwrap(),
            GatewayReply::Route(route) if route.is_some()
        ));
    }

    let reply = block_on(module.query(&encode_query(&GatewayQuery::List {
        account_id: account.account_id,
    })))
    .unwrap();
    let GatewayReply::Routes(routes) = decode_reply(&reply).unwrap() else {
        panic!("list must return routes");
    };
    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].name, RouteName::apex());
    assert_eq!(routes[1].name, RouteName::named("api"));
    assert_eq!(routes[0].revision, 1);
}

#[test]
fn handle_plane_resolves_to_the_bound_account_and_shares_the_route_gates() {
    // the merged module owns the `.duck` handle plane too: a bound, standing
    // node registers a name that resolves to its stable AccountId, and an
    // outsider is refused by the SAME valset gate the route plane uses.
    let (node, signer, account, mut context, mut module) = fixture(21);
    execute(
        &mut module,
        &mut context,
        GatewayMsg::SetHandle {
            handle: Some("orthory".into()),
        },
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();

    let reply = block_on(module.query(&encode_query(&GatewayQuery::Resolve {
        name: DuckDnsName {
            handle: "orthory".into(),
        },
    })))
    .unwrap();
    assert_eq!(
        decode_reply(&reply).unwrap(),
        GatewayReply::Resolved(Some(ResolvedAccount {
            account_id: account.account_id.clone(),
        }))
    );

    // a reserved root label is refused at admission.
    assert!(
        execute(
            &mut module,
            &mut context,
            GatewayMsg::SetHandle {
                handle: Some("net".into()),
            },
        )
        .unwrap_err()
        .to_string()
        .contains("reserved")
    );

    // the standing gate is shared: an outsider node cannot set a handle.
    let _ = signer;
    context.env.origin = Origin::External(vec![2; 32]);
    assert!(
        execute(
            &mut module,
            &mut context,
            GatewayMsg::SetHandle {
                handle: Some("intruder".into()),
            },
        )
        .unwrap_err()
        .to_string()
        .contains("validator or admitted resident")
    );
    let _ = node;
}

#[test]
fn route_revision_and_chain_prevent_replay() {
    let (node, signer, account, mut context, mut module) = fixture(9);
    let first = signed(
        statement(
            account.account_id.clone(),
            node.clone(),
            RouteName::apex(),
            1,
        ),
        &signer,
    );
    execute(&mut module, &mut context, first.clone()).unwrap();
    block_on(module.commit_block()).unwrap();
    assert!(
        execute(&mut module, &mut context, first)
            .unwrap_err()
            .to_string()
            .contains("revision")
    );

    let mut wrong_chain = statement(account.account_id, node, RouteName::apex(), 2);
    wrong_chain.chain_id = "other#12345678".into();
    assert!(
        execute(&mut module, &mut context, signed(wrong_chain, &signer))
            .unwrap_err()
            .to_string()
            .contains("another chain")
    );
}

const CHAIN_ID: &str = "test#12345678";

fn credential(name: &str, owner_account: Vec<u8>, publisher_node: Vec<u8>) -> CredentialRecord {
    CredentialRecord {
        name: name.into(),
        owner_account,
        publisher_node,
        kind: CredentialKind::Claude,
        seal_pk: [9; 32],
        grants: BTreeSet::new(),
    }
}

fn signed_set_credential(signer: &ed25519::PrivateKey, record: CredentialRecord) -> GatewayMsg {
    let statement = SetCredentialStatement {
        chain_id: CHAIN_ID.into(),
        record,
    };
    let preimage = set_credential_preimage(&statement).unwrap();
    let signature = signer
        .sign(GATEWAY_CREDENTIAL_NS, &preimage)
        .as_ref()
        .to_vec();
    GatewayMsg::SetCredential {
        statement,
        authorization: MemberAuthorization {
            signer: signer.public_key().as_ref().to_vec(),
            signature,
        },
    }
}

fn signed_remove(signer: &ed25519::PrivateKey, owner_account: Vec<u8>, name: &str) -> GatewayMsg {
    let statement = RemoveCredentialStatement {
        chain_id: CHAIN_ID.into(),
        owner_account,
        name: name.into(),
    };
    let preimage = remove_credential_preimage(&statement).unwrap();
    let signature = signer
        .sign(GATEWAY_CREDENTIAL_NS, &preimage)
        .as_ref()
        .to_vec();
    GatewayMsg::RemoveCredential {
        statement,
        authorization: MemberAuthorization {
            signer: signer.public_key().as_ref().to_vec(),
            signature,
        },
    }
}

fn grant_statement(
    owner_account: Vec<u8>,
    name: &str,
    account: Vec<u8>,
) -> CredentialGrantStatement {
    CredentialGrantStatement {
        chain_id: CHAIN_ID.into(),
        owner_account,
        name: name.into(),
        account,
    }
}

fn signed_grant(
    signer: &ed25519::PrivateKey,
    owner_account: Vec<u8>,
    name: &str,
    account: Vec<u8>,
) -> GatewayMsg {
    let statement = grant_statement(owner_account, name, account);
    let preimage = grant_credential_preimage(&statement).unwrap();
    let signature = signer
        .sign(GATEWAY_CREDENTIAL_NS, &preimage)
        .as_ref()
        .to_vec();
    GatewayMsg::GrantCredential {
        statement,
        authorization: MemberAuthorization {
            signer: signer.public_key().as_ref().to_vec(),
            signature,
        },
    }
}

fn signed_revoke(
    signer: &ed25519::PrivateKey,
    owner_account: Vec<u8>,
    name: &str,
    account: Vec<u8>,
) -> GatewayMsg {
    let statement = grant_statement(owner_account, name, account);
    let preimage = revoke_credential_preimage(&statement).unwrap();
    let signature = signer
        .sign(GATEWAY_CREDENTIAL_NS, &preimage)
        .as_ref()
        .to_vec();
    GatewayMsg::RevokeCredential {
        statement,
        authorization: MemberAuthorization {
            signer: signer.public_key().as_ref().to_vec(),
            signature,
        },
    }
}

fn query_credential(module: &Gateway, name: &str) -> Option<CredentialRecord> {
    let reply = block_on(module.query(&encode_query(&GatewayQuery::Credential {
        name: name.into(),
    })))
    .unwrap();
    match decode_reply(&reply).unwrap() {
        GatewayReply::Credential(record) => record,
        other => panic!("expected a credential reply, got {other:?}"),
    }
}

#[test]
fn credential_wire_round_trips() {
    let signer = ed25519::PrivateKey::from_seed(31);
    let record = credential("alice-claude-1", vec![5; 32], vec![1; 32]);
    let msg = signed_set_credential(&signer, record);
    let decoded = decode_msg(&encode_msg(&msg)).expect("decode");
    assert_eq!(msg, decoded);
}

#[test]
fn credential_names_are_validated() {
    let too_long = "x".repeat(65);
    for bad in ["", "UPPER", "has space", too_long.as_str()] {
        assert!(
            validate_credential_name(bad).is_err(),
            "{bad:?} must be rejected"
        );
    }
    assert!(validate_credential_name("alice-claude-1").is_ok());
}

#[test]
fn first_registration_wins_and_owner_gates_mutations() {
    let node_a = vec![1; 32];
    let node_b = vec![2; 32];
    let signer_a = ed25519::PrivateKey::from_seed(100);
    let signer_b = ed25519::PrivateKey::from_seed(200);
    let account_a = account(&node_a, &signer_a);
    let account_b = account(&node_b, &signer_b);
    let mut context = TestCtx {
        env: Env {
            height: 1,
            consensus_time: 1,
            origin: Origin::External(node_a.clone()),
            me: "gateway".into(),
        },
        validators: vec![node_a.clone(), node_b.clone()],
        residents: vec![],
        accounts: BTreeMap::from([
            (node_a.clone(), account_a.clone()),
            (node_b.clone(), account_b.clone()),
        ]),
    };
    let mut module = Gateway::new(
        "gateway",
        Box::new(sdk_testkit::MemStore::new()),
        "identity",
        Some("valset".into()),
        CHAIN_ID,
    );

    let as_a = |context: &mut TestCtx| context.env.origin = Origin::External(node_a.clone());
    let as_b = |context: &mut TestCtx| context.env.origin = Origin::External(node_b.clone());

    // owner A registers "a".
    as_a(&mut context);
    execute(
        &mut module,
        &mut context,
        signed_set_credential(
            &signer_a,
            credential("a", account_a.account_id.clone(), node_a.clone()),
        ),
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();

    // account B cannot squat the same name.
    as_b(&mut context);
    assert!(
        execute(
            &mut module,
            &mut context,
            signed_set_credential(
                &signer_b,
                credential("a", account_b.account_id.clone(), node_b.clone())
            ),
        )
        .unwrap_err()
        .to_string()
        .contains("already registered")
    );

    // a non-owner grant is refused; an owner grant commits.
    assert!(
        execute(
            &mut module,
            &mut context,
            signed_grant(
                &signer_b,
                account_b.account_id.clone(),
                "a",
                account_b.account_id.clone(),
            ),
        )
        .is_err()
    );
    as_a(&mut context);
    execute(
        &mut module,
        &mut context,
        signed_grant(
            &signer_a,
            account_a.account_id.clone(),
            "a",
            account_b.account_id.clone(),
        ),
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();
    let record = query_credential(&module, "a").expect("record");
    assert!(credential_use_allowed(&record, &account_b.account_id));

    // owner revokes, then removes.
    execute(
        &mut module,
        &mut context,
        signed_revoke(
            &signer_a,
            account_a.account_id.clone(),
            "a",
            account_b.account_id.clone(),
        ),
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();
    assert!(!credential_use_allowed(
        &query_credential(&module, "a").unwrap(),
        &account_b.account_id
    ));

    execute(
        &mut module,
        &mut context,
        signed_remove(&signer_a, account_a.account_id.clone(), "a"),
    )
    .unwrap();
    block_on(module.commit_block()).unwrap();
    assert!(query_credential(&module, "a").is_none());
}
