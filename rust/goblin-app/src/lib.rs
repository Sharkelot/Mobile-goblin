//! Native Rust Android client for the Hermes dashboard.

use futures_util::{SinkExt, StreamExt};
use goblin_core::human::{HumanRequest, approval_result, clarify_result};
use goblin_core::{GatewayClient, GatewayUrl, RpcFrame, parse_rpc_line, rpc_request, rpc_response};
use http::header::SEC_WEBSOCKET_PROTOCOL;
use serde_json::{Value, json};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::collections::VecDeque;
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
    AnswerChoice(String),
    AnswerText(String),
    CancelInput,
    Interrupt,
    RefreshProfiles,
    SelectProfile(String),
    RefreshModels,
    SelectModel {
        provider: String,
        model: String,
    },
    SetEffort(String),
    SetFast(bool),
    Steer(String),
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
    let sender = commands.clone();
    ui.on_answer_choice(move |choice| {
        let _ = sender.send(Command::AnswerChoice(choice.to_string()));
    });
    let sender = commands.clone();
    ui.on_answer_text(move |value| {
        let _ = sender.send(Command::AnswerText(value.to_string()));
    });
    let sender = commands.clone();
    ui.on_cancel_input(move || {
        let _ = sender.send(Command::CancelInput);
    });
    let sender = commands.clone();
    ui.on_interrupt(move || {
        let _ = sender.send(Command::Interrupt);
    });
    let sender = commands.clone();
    ui.on_refresh_profiles(move || {
        let _ = sender.send(Command::RefreshProfiles);
    });
    let sender = commands.clone();
    ui.on_select_profile(move |profile| {
        let _ = sender.send(Command::SelectProfile(profile.to_string()));
    });
    let sender = commands.clone();
    ui.on_refresh_models(move || {
        let _ = sender.send(Command::RefreshModels);
    });
    let sender = commands.clone();
    ui.on_select_model(move |provider, model| {
        let _ = sender.send(Command::SelectModel {
            provider: provider.to_string(),
            model: model.to_string(),
        });
    });
    let sender = commands.clone();
    ui.on_set_effort(move |effort| {
        let _ = sender.send(Command::SetEffort(effort.to_string()));
    });
    let sender = commands.clone();
    ui.on_set_fast(move |fast| {
        let _ = sender.send(Command::SetFast(fast));
    });
    let sender = commands.clone();
    ui.on_steer(move |text| {
        let _ = sender.send(Command::Steer(text.to_string()));
    });
    ui.on_send_message(move |message| {
        let _ = commands.send(Command::Send(message.to_string()));
    });
    ui.run()
}

async fn worker(mut rx: mpsc::UnboundedReceiver<Command>) {
    let mut pending_connect: Option<Command> = None;
    loop {
        let command = match pending_connect.take() {
            Some(command) => command,
            None => match rx.recv().await {
                Some(command) => command,
                None => return,
            },
        };
        let Command::Connect {
            url,
            username,
            password,
        } = command
        else {
            continue;
        };
        let mut restore_id: Option<String> = None;
        let mut conversation = String::new();
        let mut client_state = ClientState::default();
        let mut attempt = 0_u32;
        loop {
            let mut ready = false;
            status(if attempt == 0 {
                "Checking Hermes dashboard…".to_owned()
            } else {
                format!("Reconnecting to Hermes (attempt {})…", attempt + 1)
            });
            let result = run_connection(
                &mut rx,
                &url,
                &username,
                &password,
                &mut restore_id,
                &mut conversation,
                &mut client_state,
                &mut ready,
            )
            .await;
            on_ui(|ui| {
                ui.set_connected(false);
                ui.set_busy(false);
                ui.set_input_pending(false);
            });
            match result {
                Ok(()) => status("Connection closed"),
                Err(error) => status(format!("Connection lost: {error}")),
            }
            if !ready {
                break;
            }
            attempt = attempt.saturating_add(1);
            let delay = 1_u64 << attempt.min(5);
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(delay)) => {},
                command = rx.recv() => {
                    match command {
                        Some(connect @ Command::Connect { .. }) => { pending_connect = Some(connect); break; },
                        None => return,
                        _ => {},
                    }
                }
            }
        }
    }
}

async fn run_connection(
    rx: &mut mpsc::UnboundedReceiver<Command>,
    url: &str,
    username: &str,
    password: &str,
    restore_id: &mut Option<String>,
    conversation: &mut String,
    client_state: &mut ClientState,
    ready: &mut bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let gateway = GatewayClient::new(GatewayUrl::parse(url)?)?;
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
    let mut next_id: u64 = 1;
    let mut pending_send: VecDeque<String> = VecDeque::new();
    let mut attach_id: Option<String> = None;
    let mut human_inputs: VecDeque<PendingInput> = VecDeque::new();
    let mut capabilities_id: Option<String> = None;
    let mut declines_not_shown = false;
    let mut heartbeat_required = false;
    let mut last_inbound = std::time::Instant::now();
    let mut model_options_id: Option<String> = None;
    let mut profiles_id: Option<String> = None;
    let mut model_set_id: Option<String> = None;
    let mut model_candidate: Option<(String, String)> = None;
    let mut confirm_candidate: Option<(String, String)> = None;
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(15));
    heartbeat.tick().await;
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if heartbeat_required {
                    if last_inbound.elapsed() > std::time::Duration::from_secs(45) {
                        return Err("Hermes heartbeat timed out".into());
                    }
                    send_rpc(&mut sink, &mut next_id, "gateway.ping", json!({})).await?;
                }
                sink.send(Message::Ping(Vec::new().into())).await?;
            },
            command = rx.recv() => match command {
                Some(Command::RefreshSessions) => {
                    match gateway.sessions(30, 0, client_state.selected_profile.as_deref()).await {
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
                    *restore_id = Some(stored_id.clone());
                    client_state.session_profile = client_state.selected_profile.clone();
                    pending_send.clear();
                    match gateway.messages(&stored_id, client_state.selected_profile.as_deref()).await {
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
                    attach_id = Some(send_rpc(&mut sink, &mut next_id, "session.resume", json!({"session_id": stored_id, "profile": client_state.session_profile, "source": "mobile-goblin", "cols": 80, "omit_messages": true})).await?);
                    status("Opening chat…");
                }
                Some(Command::NewChat) => {
                    session_id = None;
                    *restore_id = None;
                    attach_id = None;
                    pending_send.clear();
                    client_state.session_profile = None;
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
                        send_rpc(&mut sink, &mut next_id, "prompt.submit", json!({"session_id": id, "profile": client_state.session_profile, "text": text})).await?;
                        on_ui(|ui| ui.set_busy(true));
                    } else {
                        pending_send.push_back(text);
                        if attach_id.is_none() {
                            client_state.session_profile = client_state.selected_profile.clone();
                            let model = client_state.selected_model.as_ref().map(|(_, model)| model.clone());
                            let provider = client_state.selected_model.as_ref().map(|(provider, _)| provider.clone());
                            attach_id = Some(send_rpc(&mut sink, &mut next_id, "session.create", json!({"profile":client_state.session_profile,"model":model,"provider":provider,"reasoning_effort":client_state.reasoning_effort,"fast":client_state.fast,"source": "mobile-goblin", "cols": 80})).await?);
                        }
                    }
                    status("Hermes is working…");
                }
                Some(Command::AnswerChoice(choice)) => {
                    if let Some(PendingInput { request: HumanRequest::Approval { choices, .. }, .. }) = human_inputs.front() {
                        if let Some(result) = approval_result(&choice, choices) {
                            let id = human_inputs.pop_front().unwrap().request.id().to_owned();
                            sink.send(Message::Text(rpc_response(&id, result).to_string().into())).await?;
                            render_pending(human_inputs.front());
                        }
                    }
                }
                Some(Command::AnswerText(value)) => {
                    if let Some(front) = human_inputs.front_mut() {
                        let completed = match &front.request {
                            HumanRequest::Clarify { questions, .. } => {
                                front.answers.push(value);
                                front.answers.len() == questions.len()
                            }
                            HumanRequest::Secret { .. } => { front.answers.push(value); true },
                            _ => false,
                        };
                        if completed {
                            let front = human_inputs.pop_front().unwrap();
                            let result = match &front.request {
                                HumanRequest::Clarify { questions, .. } => clarify_result(questions, &front.answers),
                                HumanRequest::Secret { .. } => Some(json!({"value": front.answers.first().cloned().unwrap_or_default()})),
                                _ => None,
                            };
                            if let Some(result) = result {
                                sink.send(Message::Text(rpc_response(front.request.id(), result).to_string().into())).await?;
                            }
                        }
                        render_pending(human_inputs.front());
                    }
                }
                Some(Command::CancelInput) => {
                    if let Some(front) = human_inputs.pop_front() {
                        let result = match front.request {
                            HumanRequest::Approval { .. } => json!({"choice":"deny"}),
                            HumanRequest::Clarify { .. } => json!({}),
                            HumanRequest::Secret { .. } => json!({"value":""}),
                        };
                        sink.send(Message::Text(rpc_response(front.request.id(), result).to_string().into())).await?;
                        render_pending(human_inputs.front());
                    }
                }
                Some(Command::Interrupt) => {
                    if let Some(id) = &session_id {
                        send_rpc(&mut sink, &mut next_id, "session.interrupt", json!({"session_id":id,"profile":client_state.session_profile})).await?;
                        status("Stopping reply…");
                    }
                }
                Some(Command::RefreshProfiles) => {
                    profiles_id = Some(send_rpc(&mut sink, &mut next_id, "profiles.list", json!({"include_sessions":false})).await?);
                }
                Some(Command::SelectProfile(profile)) => {
                    client_state.selected_profile = if profile == "default" { None } else { Some(profile.clone()) };
                    session_id = None;
                    *restore_id = None;
                    attach_id = None;
                    pending_send.clear();
                    client_state.session_profile = None;
                    client_state.selected_model = None;
                    client_state.reasoning_effort = None;
                    client_state.fast = false;
                    conversation.clear();
                    draft.clear();
                    transcript("");
                    on_ui(move |ui| {
                        ui.set_session_id("".into());
                        ui.set_selected_profile(profile.into());
                        ui.set_selected_model("Profile default".into());
                        ui.set_selected_effort("Profile default".into());
                        ui.set_fast(false);
                    });
                    status("Agent selected. Start a chat or open one of its recent chats.");
                }
                Some(Command::RefreshModels) => {
                    model_options_id = Some(send_rpc(&mut sink, &mut next_id, "model.options", json!({"profile":client_state.selected_profile,"session_id":session_id})).await?);
                }
                Some(Command::SelectModel { provider, model }) => {
                    if let Some(id) = &session_id {
                        let confirm = confirm_candidate.as_ref() == Some(&(provider.clone(), model.clone()));
                        model_candidate = Some((provider.clone(), model.clone()));
                        model_set_id = Some(send_rpc(&mut sink, &mut next_id, "config.set", json!({
                            "profile":client_state.session_profile,"session_id":id,"key":"model",
                            "value":format!("{model} --provider {provider} --session"),
                            "confirm_expensive_model":confirm
                        })).await?);
                        status(if confirm { "Confirming model cost…" } else { "Switching model…" });
                    } else {
                        client_state.selected_model = Some((provider.clone(), model.clone()));
                        on_ui(move |ui| ui.set_selected_model(format!("{provider} / {model}").into()));
                        status("Model selected for the next chat");
                    }
                }
                Some(Command::SetEffort(effort)) => {
                    if !matches!(effort.as_str(), "none"|"low"|"medium"|"high") { continue; }
                    client_state.reasoning_effort = Some(effort.clone());
                    on_ui(move |ui| ui.set_selected_effort(effort.clone().into()));
                    if let Some(id) = &session_id {
                        send_rpc(&mut sink, &mut next_id, "config.set", json!({"profile":client_state.session_profile,"session_id":id,"key":"reasoning","value":client_state.reasoning_effort})).await?;
                    }
                }
                Some(Command::SetFast(fast)) => {
                    client_state.fast = fast;
                    if let Some(id) = &session_id {
                        send_rpc(&mut sink, &mut next_id, "config.set", json!({"profile":client_state.session_profile,"session_id":id,"key":"fast","value":if fast { "fast" } else { "normal" }})).await?;
                    }
                }
                Some(Command::Steer(text)) => {
                    if let Some(id) = &session_id {
                        send_rpc(&mut sink, &mut next_id, "session.steer", json!({"profile":client_state.session_profile,"session_id":id,"text":text})).await?;
                        conversation.push_str(&format!("Steer: {text}\n\n"));
                        transcript(conversation.clone());
                        status("Steering Hermes…");
                    }
                }
                Some(Command::Connect { .. }) => status("Already connected"),
                None => return Ok(()),
            },
            frame = stream.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    last_inbound = std::time::Instant::now();
                    for line in text.lines().filter(|line| !line.trim().is_empty()) {
                        match parse_rpc_line(line) {
                            Ok(RpcFrame::Event { kind, payload, session_id: event_session, .. }) => {
                                if kind != "gateway.ready" && session_id.is_some() && event_session.as_deref().is_some_and(|id| Some(id) != session_id.as_deref()) { continue; }
                                match kind.as_str() {
                                    "gateway.ready" => {
                                        *ready = true;
                                        heartbeat_required = payload.get("heartbeat").and_then(Value::as_bool) == Some(true);
                                        if let Some(stored) = restore_id.as_ref() {
                                            attach_id = Some(send_rpc(&mut sink, &mut next_id, "session.resume", json!({"session_id":stored,"profile":client_state.session_profile,"source":"mobile-goblin","cols":80,"omit_messages":true})).await?);
                                        }
                                        on_ui(|ui| ui.set_connected(true));
                                        status("Connected to Hermes");
                                        capabilities_id = Some(send_rpc(&mut sink, &mut next_id, "client.capabilities", json!({"server_requests": true})).await?);
                                    }
                                    "request.cancel" | "approval.cancelled" => {
                                        if let Some(id) = payload.get("id").or_else(|| payload.get("request_id")).and_then(Value::as_str) {
                                            human_inputs.retain(|pending| pending.request.id() != id);
                                            render_pending(human_inputs.front());
                                        }
                                    }
                                    "tool.start" => status(format!("Using {}…", payload.get("name").and_then(Value::as_str).unwrap_or("tool"))),
                                    "tool.complete" => status("Hermes is working…"),
                                    "message.delta" => {
                                        if let Some(chunk) = payload.get("text").or_else(|| payload.get("delta")).and_then(Value::as_str) {
                                            draft.push_str(chunk);
                                            transcript(format!("{conversation}{draft}"));
                                        }
                                    }
                                    "message.start" => on_ui(|ui| ui.set_busy(true)),
                                    "message.complete" => {
                                        on_ui(|ui| ui.set_busy(false));
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
                            Ok(RpcFrame::Response { id: response_id, result }) => match result {
                                Ok(value) => {
                                    if profiles_id.as_deref() == Some(&response_id) {
                                        let rows: Vec<(String, String)> = value.get("profiles").and_then(Value::as_array)
                                            .into_iter().flatten().filter_map(|item| {
                                                let name = item.get("name")?.as_str()?.to_owned();
                                                let label = item.get("display_name").and_then(Value::as_str).unwrap_or(&name).to_owned();
                                                Some((name, label))
                                            }).collect();
                                        on_ui(move |ui| {
                                            let model = rows.into_iter().map(|(name,label)| ProfileRow { name:name.into(), label:label.into() }).collect::<Vec<_>>();
                                            ui.set_profiles(slint::ModelRc::new(slint::VecModel::from(model)));
                                        });
                                        continue;
                                    }
                                    if model_options_id.as_deref() == Some(&response_id) {
                                        let mut rows = Vec::new();
                                        for provider in value.get("providers").and_then(Value::as_array).into_iter().flatten() {
                                            if provider.get("authenticated").and_then(Value::as_bool) == Some(false) { continue; }
                                            let Some(slug) = provider.get("slug").and_then(Value::as_str) else { continue; };
                                            for model in provider.get("models").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                                                rows.push((slug.to_owned(), model.to_owned()));
                                            }
                                        }
                                        on_ui(move |ui| {
                                            let model = rows.into_iter().map(|(provider,id)| ModelRow { label:format!("{provider} / {id}").into(), provider:provider.into(), id:id.into() }).collect::<Vec<_>>();
                                            ui.set_models(slint::ModelRc::new(slint::VecModel::from(model)));
                                        });
                                        continue;
                                    }
                                    if model_set_id.as_deref() == Some(&response_id) {
                                        if value.get("confirm_required").and_then(Value::as_bool) == Some(true) {
                                            confirm_candidate = model_candidate.clone();
                                            status(value.get("confirm_message").and_then(Value::as_str).unwrap_or("This model may cost more. Select it again to confirm."));
                                        } else if let Some((provider, model)) = model_candidate.take() {
                                            confirm_candidate = None;
                                            client_state.selected_model = Some((provider.clone(), model.clone()));
                                            on_ui(move |ui| ui.set_selected_model(format!("{provider} / {model}").into()));
                                            status("Model switched");
                                        }
                                        continue;
                                    }
                                    if capabilities_id.as_deref() == Some(&response_id) {
                                        declines_not_shown = value.get("declines_not_shown").and_then(Value::as_bool) == Some(true);
                                        continue;
                                    }
                                    if attach_id.as_deref() == Some(&response_id) {
                                        attach_id = None;
                                        if let Some(id) = value.get("session_id").and_then(Value::as_str) {
                                            session_id = Some(id.to_owned());
                                            for item in value.get("open_requests").and_then(Value::as_array).into_iter().flatten() {
                                                let (Some(request_id), Some(method)) = (item.get("id").and_then(Value::as_str), item.get("method").and_then(Value::as_str)) else { continue; };
                                                if human_inputs.iter().any(|pending| pending.request.id() == request_id) { continue; }
                                                let params = item.get("params").cloned().unwrap_or_else(|| json!({}));
                                                if let Some(request) = HumanRequest::parse(request_id.to_owned(), method, &params) {
                                                    human_inputs.push_back(PendingInput { request, answers: Vec::new() });
                                                }
                                            }
                                            render_pending(human_inputs.front());
                                            if let Some(stored) = value.get("stored_session_id").and_then(Value::as_str) {
                                                *restore_id = Some(stored.to_owned());
                                            }
                                            on_ui(|ui| ui.set_session_id("active".into()));
                                            let mut queued = false;
                                            while let Some(text) = pending_send.pop_front() {
                                                send_rpc(&mut sink, &mut next_id, "prompt.submit", json!({"session_id": id, "profile": client_state.session_profile, "text": text, "queued": queued})).await?;
                                                queued = true;
                                            }
                                        }
                                    }
                                }
                                Err(fault) => status(format!("Hermes: {}", fault.message)),
                            },
                            Ok(RpcFrame::ServerRequest { id, method, params }) => {
                                if let Some(request) = HumanRequest::parse(id.clone(), &method, &params) {
                                    human_inputs.push_back(PendingInput { request, answers: Vec::new() });
                                    if human_inputs.len() == 1 { render_pending(human_inputs.front()); }
                                } else if declines_not_shown {
                                    let decline = json!({"jsonrpc":"2.0", "id":id, "error":{"code":4404,"message":"Mobile Goblin cannot display this request"}});
                                    sink.send(Message::Text(decline.to_string().into())).await?;
                                }
                            }
                            Err(error) => status(format!("Protocol error: {error}")),
                        }
                    }
                }
                Some(Ok(Message::Ping(bytes))) => { last_inbound = std::time::Instant::now(); sink.send(Message::Pong(bytes)).await?; },
                Some(Ok(Message::Close(_))) | None => return Ok(()),
                Some(Ok(_)) => {},
                Some(Err(error)) => return Err(error.into()),
            }
        }
    }
}

#[derive(Default)]
struct ClientState {
    selected_profile: Option<String>,
    session_profile: Option<String>,
    selected_model: Option<(String, String)>,
    reasoning_effort: Option<String>,
    fast: bool,
}

struct PendingInput {
    request: HumanRequest,
    answers: Vec<String>,
}

fn render_pending(input: Option<&PendingInput>) {
    let Some(input) = input else {
        on_ui(|ui| ui.set_input_pending(false));
        return;
    };
    let (title, detail, approval, masked, session, always) = match &input.request {
        HumanRequest::Approval {
            description,
            command,
            choices,
            ..
        } => (
            "Approval needed".to_owned(),
            format!("{description}\n{command}"),
            true,
            false,
            choices.iter().any(|c| c == "session"),
            choices.iter().any(|c| c == "always"),
        ),
        HumanRequest::Clarify { questions, .. } => {
            let index = input.answers.len().min(questions.len().saturating_sub(1));
            let q = &questions[index];
            let hint = if q.choices.is_empty() {
                String::new()
            } else {
                format!("\nOptions: {}", q.choices.join(", "))
            };
            (
                format!("Question {} of {}", index + 1, questions.len()),
                format!(
                    "{}{}{}",
                    q.text,
                    hint,
                    if q.multi_select {
                        "\nSelect multiple by separating answers with commas."
                    } else {
                        ""
                    }
                ),
                false,
                false,
                false,
                false,
            )
        }
        HumanRequest::Secret { prompt, masked, .. } => (
            "Hermes needs a value".to_owned(),
            prompt.clone(),
            false,
            *masked,
            false,
            false,
        ),
    };
    let detail = match input.request.session_id() {
        Some(id) => format!(
            "Session {}\n{}",
            id.chars().take(12).collect::<String>(),
            detail
        ),
        None => detail,
    };
    on_ui(move |ui| {
        ui.set_input_title(title.into());
        ui.set_input_detail(detail.into());
        ui.set_input_approval(approval);
        ui.set_input_masked(masked);
        ui.set_allow_session(session);
        ui.set_allow_always(always);
        ui.set_input_pending(true);
    });
}

async fn send_rpc<S>(
    sink: &mut S,
    next_id: &mut u64,
    method: &str,
    params: Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>>
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
    Ok(id)
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
