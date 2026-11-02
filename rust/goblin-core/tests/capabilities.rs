use goblin_core::{GatewayClient, GatewayUrl};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn capability_routes_follow_kyllene_profile_and_body() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for reply in ["[]", "{}"] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let read = stream.read(&mut chunk).await.unwrap();
                assert!(read > 0);
                request.extend_from_slice(&chunk[..read]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]);
                    let length: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|v| v.parse().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),
                reply
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            requests.push(String::from_utf8(request).unwrap());
        }
        requests
    });
    let client =
        GatewayClient::new(GatewayUrl::parse(&format!("http://{address}/hermes")).unwrap())
            .unwrap();
    client.capabilities("skills", Some("artist")).await.unwrap();
    client
        .set_capability_enabled("toolsets", "web/search", false, Some("artist"))
        .await
        .unwrap();
    let requests = server.await.unwrap();
    assert!(requests[0].starts_with("GET /hermes/api/skills?profile=artist HTTP/1.1"));
    assert!(
        requests[1]
            .starts_with("PUT /hermes/api/tools/toolsets/web%2Fsearch?profile=artist HTTP/1.1")
    );
    assert!(requests[1].contains("\"enabled\":false"));
}
