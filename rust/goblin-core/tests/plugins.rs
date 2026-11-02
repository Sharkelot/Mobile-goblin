use goblin_core::{GatewayClient, GatewayUrl};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn plugin_catalogue_uses_kyllene_route_and_profile() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        let bytes = stream.read(&mut request).await.unwrap();
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]";
        stream.write_all(response.as_bytes()).await.unwrap();
        String::from_utf8(request[..bytes].to_vec()).unwrap()
    });
    let client =
        GatewayClient::new(GatewayUrl::parse(&format!("http://{address}/hermes")).unwrap())
            .unwrap();
    assert_eq!(
        client.dashboard_plugins(Some("artist")).await.unwrap(),
        serde_json::json!([])
    );
    let request = server.await.unwrap();
    assert!(request.starts_with("GET /hermes/api/dashboard/plugins?profile=artist HTTP/1.1"));
}
