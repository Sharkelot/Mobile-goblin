use goblin_core::{
    GatewayUrl, GoblinError, RpcFrame, is_private_host, parse_rpc_line, rpc_request,
};
use serde_json::json;

#[test]
fn normalizes_gateway_and_keeps_path_prefix() {
    let url = GatewayUrl::parse("  goblin.example.ts.net:10443/hermes/?old=1#top ").unwrap();
    assert_eq!(url.as_str(), "http://goblin.example.ts.net:10443/hermes");
    assert_eq!(
        url.resolve("api/status").unwrap().as_str(),
        "http://goblin.example.ts.net:10443/hermes/api/status"
    );
    assert_eq!(
        url.websocket_url().unwrap().as_str(),
        "ws://goblin.example.ts.net:10443/hermes/api/ws"
    );
}

#[test]
fn classifies_private_and_public_hosts() {
    for host in [
        "localhost",
        "127.0.0.1",
        "192.168.1.10",
        "100.122.170.104",
        "node.ts.net",
        "fd7a:115c:a1e0::1",
    ] {
        assert!(is_private_host(host), "{host}");
    }
    for host in ["example.com", "8.8.8.8", "100.128.0.1"] {
        assert!(!is_private_host(host), "{host}");
    }
    assert!(
        GatewayUrl::parse("http://example.com:9119")
            .unwrap()
            .is_exposed()
    );
    assert!(
        !GatewayUrl::parse("https://example.com")
            .unwrap()
            .is_exposed()
    );
}

#[test]
fn parses_gateway_events_and_server_requests() {
    let event = parse_rpc_line(r#"{"jsonrpc":"2.0","method":"event","params":{"type":"message.delta","payload":{"text":"hi"},"session_id":"abc","seq":7}}"#).unwrap();
    assert_eq!(
        event,
        RpcFrame::Event {
            kind: "message.delta".into(),
            payload: json!({"text":"hi"}),
            session_id: Some("abc".into()),
            seq: Some(7)
        }
    );
    let request = parse_rpc_line(
        r#"{"jsonrpc":"2.0","id":"srq-7","method":"approval","params":{"command":"ls"}}"#,
    )
    .unwrap();
    assert_eq!(
        request,
        RpcFrame::ServerRequest {
            id: "srq-7".into(),
            method: "approval".into(),
            params: json!({"command":"ls"})
        }
    );
    assert_eq!(
        rpc_request("c1", "client.capabilities", json!({"server_requests":true}))["method"],
        "client.capabilities"
    );
}

#[test]
fn rejects_malformed_frames() {
    assert!(matches!(
        parse_rpc_line("[]"),
        Err(GoblinError::InvalidFrame(_))
    ));
    assert!(matches!(
        parse_rpc_line(r#"{"id":"c1","result":{}}"#),
        Err(GoblinError::InvalidFrame(_))
    ));
}
