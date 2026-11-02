//! Native Rust Android client for the Hermes dashboard.

use futures_util::{SinkExt, StreamExt};
use goblin_core::{GatewayClient, GatewayUrl, RpcFrame, parse_rpc_line, rpc_request};
use http::header::SEC_WEBSOCKET_PROTOCOL;
use serde_json::{Value, json};
use slint::ComponentHandle;
use std::cell::RefCell;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

slint::include_modules!();

enum Command {
    Connect {
        url: String,
        username: String,
        password: String,
    },
    NewChat,
    RefreshSessions,
    OpenSession(String),
    Send(String),
}

thread_local! {
    static UI: RefCell<Option<slint::Weak<MainWindow>>> = const { RefCell::new(None) };
}

fn on_ui(f: impl FnOnce(&MainWindow) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || {
        UI.with(|cell| {
            if let Some(ui) = cell.borrow().as_ref().and_then(|weak| weak.upgrade()) {
                f(&ui);
            }
        });
    });
}

fn status(message: impl Into<String>) {
    let message = message.into();
    on_ui(move |ui| ui.set_status(message.into()));
}

fn transcript(message: impl Into<String>) {
    let message = message.into();
    on_ui(move |ui| ui.set_conversation(message.into()));
}

fn start() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    UI.with(|cell| *cell.borrow_mut() = Some(ui.as_weak()));
    let (commands, rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        if let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            runtime.block_on(worker(rx));
        } else {
            status("Could not start network runtime");
        }
    });

    let sender = commands.clone();
    ui.on_connect(move |url, username, password| {
        let _ = sender.send(Command::Connect {
            url: url.to_string(),
            username: username.to_string(),
            password: password.to_string(),
        });
    });
    let sender = commands.clone();
    ui.on_refresh_sessions(move || {
        let _ = sender.send(Command::RefreshSessions);
    });
    let sender = commands.clone();
    ui.on_open_session(move |id| {
        let _ = sender.send(Command::OpenSession(id.to_string()));
    });
    let sender = commands.clone();
    ui.on_new_chat(move || {
        let _ = sender.send(Command::NewChat);
    });
    ui.on_send_message(move |message| {
        let _ = commands.send(Command::Send(message.to_string()));
    });
    ui.run()
}

async fn worker(mut rx: mpsc::UnboundedReceiver<Command>) {
    while let Some(command) = rx.recv().await {
        let Command::Connect {
            url,
            username,
            password,
        } = command
        else {
            continue;
        };
        status("Checking Hermes dashboard…");
        match run_connection(&mut rx, url, username, password).await {
            Ok(()) => status("Disconnected"),
            Err(error) => status(format!("Connection failed: {error}")),
        }
        on_ui(|ui| ui.set_connected(false));
    }
}

async fn run_connection(
    rx: &mut mpsc::UnboundedReceiver<Command>,
    url: String,
    username: String,
    password: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let gateway = GatewayClient::new(GatewayUrl::parse(&url)?)?;
    let info = gateway.status().await?;
    if info.auth_required {
        if username.trim().is_empty() || password.is_empty() {
            return Err("Username and password are required by this gateway".into());
        }
        status("Signing in…");
        gateway
            .password_login("basic", &username, &password)
            .await?;
    }
    let ticket = gateway.ws_ticket().await?;
    let mut request = gateway
        .url
        .websocket_url()?
        .as_str()
        .into_client_request()?;
    request.headers_mut().insert(
        SEC_WEBSOCKET_PROTOCOL,
        format!("hermes-gateway-v1, hermes-gateway-ticket.{}", ticket.ticket).parse()?,
    );
    status("Opening live connection…");
    let (socket, _) = connect_async(request).await?;
    let (mut sink, mut stream) = socket.split();
    let mut session_id: Option<String> = None;
    let mut draft = String::new();
    let mut conversation = String::new();
    let mut next_id: u64 = 1;
    let mut pending_send: Option<String> = None;
    on_ui(|ui| ui.set_connected(true));
    status("Connected to Hermes");
    loop {
        tokio::select! {
            command = rx.recv() => match command {
                Some(Command::RefreshSessions) => {
                    match gateway.sessions(30, 0).await {
                        Ok(page) => {
                            let rows: Vec<(String, String)> = page.get("sessions").and_then(Value::as_array)
                                .into_iter().flatten().filter_map(|item| {
                                    let id = item.get("id")?.as_str()?.to_owned();
                                    let title = item.get("title").or_else(|| item.get("preview"))
                                        .and_then(Value::as_str).unwrap_or("Untitled chat")
                                        .lines().next().unwrap_or("Untitled chat").to_owned();
                                    Some((id, title))
                                }).collect();
                            on_ui(move |ui| {
                                let model = rows.into_iter().map(|(id, title)| SessionRow { id: id.into(), title: title.into() }).collect::<Vec<_>>();
                                ui.set_sessions(slint::ModelRc::new(slint::VecModel::from(model)));
                            });
                            status("Choose a chat");
                        }
                        Err(error) => status(format!("Cannot load chats: {error}")),
                    }
                }
                Some(Command::OpenSession(stored_id)) => {
                    conversation.clear();
                    draft.clear();
                    session_id = None;
                    pending_send = None;
                    match gateway.messages(&stored_id).await {
                        Ok(page) => {
                            if let Some(messages) = page.get("messages").and_then(Value::as_array) {
                                for row in messages {
                                    if row.get("display_kind").and_then(Value::as_str) == Some("hidden") { continue; }
                                    let role = match row.get("role").and_then(Value::as_str) {
                                        Some("user") => "You", Some("assistant") => "Goblin", _ => continue,
                                    };
                                    let content = row.get("display_content").or_else(|| row.get("content"));
                                    let text = content.and_then(Value::as_str).unwrap_or_default();
                                    if !text.is_empty() { conversation.push_str(&format!("{role}: {text}\n\n")); }
                                }
                            }
                            transcript(conversation.clone());
                        }
                        Err(error) => status(format!("Cannot load history: {error}")),
                    }
                    send_rpc(&mut sink, &mut next_id, "session.resume", json!({"session_id": stored_id, "source": "mobile-goblin", "cols": 80, "omit_messages": true})).await?;
                    status("Opening chat…");
                }
                Some(Command::NewChat) => {
                    session_id = None;
                    draft.clear();
                    conversation.clear();
                    transcript("");
                    on_ui(|ui| ui.set_session_id("".into()));
                    status("New chat ready");
                }
                Some(Command::Send(text)) => {
                    if text.trim().is_empty() { continue; }
                    conversation.push_str(&format!("You: {}\n\nGoblin: ", text.trim()));
                    transcript(conversation.clone());
                    draft.clear();
                    if let Some(id) = &session_id {
                        send_rpc(&mut sink, &mut next_id, "prompt.submit", json!({"session_id": id, "text": text})).await?;
                    } else {
                        pending_send = Some(text);
                        send_rpc(&mut sink, &mut next_id, "session.create", json!({"source": "mobile-goblin", "cols": 80})).await?;
                    }
                    status("Hermes is working…");
                }
                Some(Command::Connect { .. }) => status("Already connected"),
                None => return Ok(()),
            },
            frame = stream.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    for line in text.lines().filter(|line| !line.trim().is_empty()) {
                        match parse_rpc_line(line) {
                            Ok(RpcFrame::Event { kind, payload, .. }) => {
                                match kind.as_str() {
                                    "gateway.ready" => {
                                        // This version does not yet render approval or secret prompts.
                                        send_rpc(&mut sink, &mut next_id, "client.capabilities", json!({"server_requests": false})).await?;
                                    }
                                    "message.delta" => {
                                        if let Some(chunk) = payload.get("text").or_else(|| payload.get("delta")).and_then(Value::as_str) {
                                            draft.push_str(chunk);
                                            transcript(format!("{conversation}{draft}"));
                                        }
                                    }
                                    "message.complete" => {
                                        if let Some(full) = payload.get("text").and_then(Value::as_str) {
                                            draft = full.to_owned();
                                        }
                                        if !draft.is_empty() {
                                            conversation.push_str(&draft);
                                            conversation.push_str("\n\n");
                                            draft.clear();
                                            transcript(conversation.clone());
                                        }
                                        status("Ready");
                                    }
                                    "error" => status(format!("Hermes error: {}", payload.get("message").and_then(Value::as_str).unwrap_or("unknown"))),
                                    _ => {}
                                }
                            }
                            Ok(RpcFrame::Response { result, .. }) => match result {
                                Ok(value) => {
                                    if session_id.is_none() {
                                        if let Some(id) = value.get("session_id").and_then(Value::as_str) {
                                            session_id = Some(id.to_owned());
                                            on_ui(|ui| ui.set_session_id("active".into()));
                                            if let Some(text) = pending_send.take() {
                                                send_rpc(&mut sink, &mut next_id, "prompt.submit", json!({"session_id": id, "text": text})).await?;
                                            }
                                        }
                                    }
                                }
                                Err(fault) => status(format!("Hermes: {}", fault.message)),
                            },
                            Ok(RpcFrame::ServerRequest { id, .. }) => {
                                let decline = json!({"jsonrpc":"2.0", "id":id, "error":{"code":-32601,"message":"Mobile Goblin cannot display this request yet"}});
                                sink.send(Message::Text(decline.to_string().into())).await?;
                            }
                            Err(error) => status(format!("Protocol error: {error}")),
                        }
                    }
                }
                Some(Ok(Message::Ping(bytes))) => sink.send(Message::Pong(bytes)).await?,
                Some(Ok(Message::Close(_))) | None => return Ok(()),
                Some(Ok(_)) => {},
                Some(Err(error)) => return Err(error.into()),
            }
        }
    }
}

async fn send_rpc<S>(
    sink: &mut S,
    next_id: &mut u64,
    method: &str,
    params: Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: SinkExt<Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    let id = format!("goblin-{}", *next_id);
    *next_id += 1;
    sink.send(Message::Text(
        rpc_request(&id, method, params).to_string().into(),
    ))
    .await?;
    Ok(())
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    slint::android::init(app).expect("initialize Android UI");
    start().expect("run Mobile Goblin");
}

#[cfg(not(target_os = "android"))]
pub fn main() {
    start().expect("run Mobile Goblin");
}
