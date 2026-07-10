use std::collections::BTreeMap;

use commonware_cryptography::{Signer as _, ed25519};
use futures::executor::block_on;
use gateway::{
    GATEWAY_ROUTE_NS, Gateway, GatewayMsg, GatewayQuery, GatewayReply, MemberAuthorization,
    ROUTE_FORMAT_VERSION, RouteAudience, RouteDefinition, RouteMethod, RouteName, RoutePolicy,
    RouteStatement, RouteTarget, decode_reply, encode_msg, encode_query, route_signing_preimage,
};
use identity::{AccountView, IdentityQuery, IdentityReply, KeyKind, MemberKeyView};
use sdk::{Ctx, Effect, Env, Error, Event, Module, Msg, Origin, StateRoot};
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
            },
            _ => Err(Error::UnknownModule(target.into())),
        }
    }

    fn emit_msg(&mut self, _msg: Msg) {}
    fn emit_event(&mut self, _event: Event) {}
    fn request_effect(&mut self, _effect: Effect) {}
}

fn account(node: &[u8], signer: &ed25519::PrivateKey) -> AccountView {
    AccountView {
        account_id: signer.public_key().as_ref().to_vec(),
        display_name: Some("Alice".into()),
        nonce: 0,
        member_keys: vec![MemberKeyView {
            pubkey: signer.public_key().as_ref().to_vec(),
            kind: KeyKind::Ed25519,
            label: None,
            added_at: 0,
        }],
        nodes: vec![node.to_vec()],
        updated_at: 0,
    }
}

fn statement(account_id: Vec<u8>, node: Vec<u8>, name: RouteName, revision: u64) -> RouteStatement {
    RouteStatement {
        version: ROUTE_FORMAT_VERSION,
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
            protocol_version: 0,
        },
        validators: vec![node.clone()],
        residents: vec![],
        accounts: BTreeMap::from([(node.clone(), account.clone())]),
    };
    let module = Gateway::new(
        "gateway",
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
        GatewayReply::Route(Some(_))
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
    let GatewayMsg::SetRoute { authorization, .. } = &mut forged;
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
            GatewayReply::Route(Some(_))
        ));
    }
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
