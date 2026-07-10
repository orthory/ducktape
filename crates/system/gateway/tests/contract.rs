use gateway::{
    GatewayReply, ROUTE_FORMAT_VERSION, RouteAudience, RouteDefinition, RouteMethod, RouteName,
    RoutePolicy, RouteStatement, RouteSummary, RouteTarget, RouteTargetKind,
    route_signing_preimage,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn statement() -> RouteStatement {
    RouteStatement {
        version: ROUTE_FORMAT_VERSION,
        chain_id: "test".into(),
        account_id: vec![1, 2],
        name: RouteName::named("api"),
        publisher_node: vec![3; 32],
        revision: 7,
        route: Some(RouteDefinition {
            target: RouteTarget::LoopbackHttp,
            policy: RoutePolicy {
                audience: RouteAudience::Network,
                methods: vec![RouteMethod::Get, RouteMethod::Head, RouteMethod::Post],
                max_request_bytes: 1024,
                max_response_bytes: 4096,
                allow_authorization: false,
            },
        }),
    }
}

#[test]
fn signing_preimage_has_a_cross_language_fixed_vector() {
    let encoded = hex(&route_signing_preimage(&statement()).unwrap());
    assert_eq!(
        encoded,
        "010400000000000000746573740200000000000000010201030000000000000061706920000000000000000303030303030303030303030303030303030303030303030303030303030303070000000000000001020300000000000000010203000400000000000000100000000000000002"
    );
}

#[test]
fn route_json_rejects_unknown_fields() {
    let mut value = serde_json::to_value(statement()).unwrap();
    value["ambient_authority"] = serde_json::json!(true);
    assert!(serde_json::from_value::<RouteStatement>(value).is_err());
}

#[test]
fn management_replies_keep_the_small_external_json_shape() {
    assert_eq!(
        serde_json::to_value(GatewayReply::Route(Box::new(None))).unwrap(),
        serde_json::json!({ "route": null })
    );
    let summary = RouteSummary {
        name: RouteName::named("api"),
        publisher_node: vec![3; 32],
        revision: 7,
        target: RouteTargetKind::LoopbackHttp,
    };
    assert_eq!(
        serde_json::to_value(GatewayReply::Routes(vec![summary])).unwrap(),
        serde_json::json!({
            "routes": [{
                "name": { "label": "api" },
                "publisher_node": vec![3; 32],
                "revision": 7,
                "target": "loopback_http"
            }]
        })
    );
}
