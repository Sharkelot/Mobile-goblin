use goblin_core::{GatewayClient, GatewayUrl};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn reconnect_reuses_cookie_and_reauthenticates_only_when_it_expires() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for (index, body, extra) in [
            (0, "{}", "Set-Cookie: session=first; Path=/; HttpOnly\r\n"),
            (1, r#"{"ticket":"one"}"#, ""),
            (2, r#"{"ticket":"two"}"#, ""),
            (3, "{}", ""),
            (4, "{}", "Set-Cookie: session=second; Path=/; HttpOnly\r\n"),
            (5, r#"{"ticket":"three"}"#, ""),
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let len = stream.read(&mut chunk).await.unwrap();
                assert!(len > 0);
                bytes.extend_from_slice(&chunk[..len]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let length: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|value| value.parse().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let status = if index == 3 {
                "401 Unauthorized"
            } else {
                "200 OK"
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            requests.push(String::from_utf8(bytes).unwrap());
        }
        requests
    });
    let client =
        GatewayClient::new(GatewayUrl::parse(&format!("http://{address}")).unwrap()).unwrap();
    assert_eq!(
        client
            .ws_ticket_with_password(true, "user", "pass", false)
            .await
            .unwrap()
            .ticket,
        "one"
    );
    assert_eq!(
        client
            .ws_ticket_with_password(true, "user", "pass", true)
            .await
            .unwrap()
            .ticket,
        "two"
    );
    assert_eq!(
        client
            .ws_ticket_with_password(true, "user", "pass", true)
            .await
            .unwrap()
            .ticket,
        "three"
    );
    let requests = server.await.unwrap();
    assert!(requests[0].starts_with("POST /auth/password-login HTTP/1.1"));
    assert!(
        requests[1]
            .to_ascii_lowercase()
            .contains("cookie: session=first")
    );
    assert!(
        requests[2]
            .to_ascii_lowercase()
            .contains("cookie: session=first")
    );
    assert!(
        requests[3]
            .to_ascii_lowercase()
            .contains("cookie: session=first")
    );
    assert!(requests[4].starts_with("POST /auth/password-login HTTP/1.1"));
    assert!(
        requests[5]
            .to_ascii_lowercase()
            .contains("cookie: session=second")
    );
}
