//! Native Rust Android client for the Hermes dashboard.

use futures_util::{SinkExt, StreamExt};
use goblin_core::human::{HumanRequest, approval_result, clarify_result};
use goblin_core::{GatewayClient, GatewayUrl, RpcFrame, parse_rpc_line, rpc_request, rpc_response};
use http::header::SEC_WEBSOCKET_PROTOCOL;
use serde_json::{Value, json};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
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
    SearchSessions(String),
    ArchivedSessions,
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
    LoadResource(String),
    ManageJob {
        action: String,
        name: String,
        schedule: String,
        prompt: String,
    },
    TitleChat(String),
    ArchiveChat,
    BranchChat(String),
    SkillAction {
        action: String,
        query: String,
    },
    ToolAction {
        action: String,
        name: String,
    },
    McpTest(String),
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

fn render_chat(rows: &[ChatRow]) {
    let rows = rows.to_vec();
    on_ui(move |ui| {
        ui.set_chat_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
        ui.set_chat_revision(ui.get_chat_revision().wrapping_add(1));
    });
}

fn chat_row(kind: &str, label: &str, body: impl Into<String>) -> ChatRow {
    ChatRow {
        kind: kind.into(),
        label: label.into(),
        body: body.into().into(),
    }
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
    ui.on_search_sessions(move |query| {
        let _ = sender.send(Command::SearchSessions(query.to_string()));
    });
    let sender = commands.clone();
    ui.on_archived_sessions(move || {
        let _ = sender.send(Command::ArchivedSessions);
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
    let sender = commands.clone();
    ui.on_load_resource(move |resource| {
        let _ = sender.send(Command::LoadResource(resource.to_string()));
    });
    let sender = commands.clone();
    ui.on_manage_job(move |action, name, schedule, prompt| {
        let _ = sender.send(Command::ManageJob {
            action: action.to_string(),
            name: name.to_string(),
            schedule: schedule.to_string(),
            prompt: prompt.to_string(),
        });
    });
    let sender = commands.clone();
    ui.on_title_chat(move |title| {
        let _ = sender.send(Command::TitleChat(title.to_string()));
    });
    let sender = commands.clone();
    ui.on_archive_chat(move || {
        let _ = sender.send(Command::ArchiveChat);
    });
    let sender = commands.clone();
    ui.on_branch_chat(move |name| {
        let _ = sender.send(Command::BranchChat(name.to_string()));
    });
    let sender = commands.clone();
    ui.on_skill_action(move |action, query| {
        let _ = sender.send(Command::SkillAction {
            action: action.to_string(),
            query: query.to_string(),
        });
    });
    let sender = commands.clone();
    ui.on_tool_action(move |action, name| {
        let _ = sender.send(Command::ToolAction {
            action: action.to_string(),
            name: name.to_string(),
        });
    });
    let sender = commands.clone();
    ui.on_mcp_test(move |name| {
        let _ = sender.send(Command::McpTest(name.to_string()));
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
        let mut chat_rows = Vec::new();
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
                &mut chat_rows,
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
    chat_rows: &mut Vec<ChatRow>,
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
    let mut streaming_row: Option<usize> = None;
    let mut reasoning_row: Option<usize> = None;
    let mut reasoning_draft = String::new();
    let mut tool_rows: HashMap<String, usize> = HashMap::new();
    let mut next_id: u64 = 1;
    let mut pending_send: VecDeque<String> = VecDeque::new();
    let mut attach_id: Option<String> = None;
    let mut pending_prompts: HashSet<String> = HashSet::new();
    let mut human_inputs: VecDeque<PendingInput> = VecDeque::new();
    let mut capabilities_id: Option<String> = None;
    let mut declines_not_shown = false;
    let mut heartbeat_required = false;
    let mut last_inbound = std::time::Instant::now();
    let mut model_options_id: Option<String> = None;
    let mut profiles_id: Option<String> = None;
    let mut model_set_id: Option<String> = None;
    let mut resource_requests: HashMap<String, String> = HashMap::new();
    let mut job_mutation_ids: HashSet<String> = HashSet::new();
    let mut skill_install_ids: HashSet<String> = HashSet::new();
    let mut tool_action_ids: HashSet<String> = HashSet::new();
    let mut branch_id: Option<String> = None;
    let mut archive_id: Option<String> = None;
    let mut title_id: Option<String> = None;
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
                            present_sessions(&page, "sessions");
                            status("Recent chats");
                        }
                        Err(error) => status(format!("Cannot load chats: {error}")),
                    }
                }
                Some(Command::SearchSessions(query)) => {
                    if query.trim().is_empty() {
                        match gateway.sessions(30, 0, client_state.selected_profile.as_deref()).await {
                            Ok(page) => { present_sessions(&page, "sessions"); status("Recent chats"); }
                            Err(error) => status(format!("Cannot load chats: {error}")),
                        }
                        continue;
                    }
                    status("Searching chats…");
                    match gateway.search_sessions(&query, client_state.selected_profile.as_deref()).await {
                        Ok(page) => {
                            let count = present_sessions(&page, "results");
                            status(format!("{count} chat{} found", if count == 1 { "" } else { "s" }));
                        }
                        Err(error) => status(format!("Cannot search chats: {error}")),
                    }
                }
                Some(Command::ArchivedSessions) => {
                    status("Loading archived chats…");
                    match gateway.archived_sessions(client_state.selected_profile.as_deref()).await {
                        Ok(page) => {
                            let count = present_sessions(&page, "sessions");
                            status(format!("{count} archived chat{}", if count == 1 { "" } else { "s" }));
                        }
                        Err(error) => status(format!("Cannot load archived chats: {error}")),
                    }
                }
                Some(Command::OpenSession(stored_id)) => {
                    chat_rows.clear();
                    tool_rows.clear();
                    reasoning_row = None;
                    reasoning_draft.clear();
                    render_chat(chat_rows);
                    draft.clear();
                    streaming_row = None;
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
                                        Some("user") => "You", Some("assistant") => "Goblin", Some("tool") => "Tool", _ => continue,
                                    };
                                    let content = row.get("display_content").or_else(|| row.get("content"));
                                    let text = history_text(content);
                                    let kind = match role { "You" => "user", "Goblin" => "assistant", _ => "tool" };
                                    if kind == "assistant" {
                                        let reasoning = history_reasoning(row);
                                        if !reasoning.is_empty() {
                                            chat_rows.push(chat_row("reasoning", "Reasoning", reasoning));
                                        }
                                    }
                                    if !text.is_empty() {
                                        let label = if kind == "tool" { row.get("tool_name").and_then(Value::as_str).unwrap_or("Tool") } else { role };
                                        chat_rows.push(chat_row(kind, label, text));
                                    }
                                    if kind == "assistant" {
                                        if let Some(calls) = row.get("tool_calls").and_then(Value::as_array) {
                                            for call in calls {
                                                let name = call.pointer("/function/name").or_else(|| call.get("name")).and_then(Value::as_str).unwrap_or("Tool call");
                                                chat_rows.push(chat_row("tool", name, "Called by Hermes"));
                                            }
                                        }
                                    }
                                }
                            }
                            render_chat(chat_rows);
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
                    streaming_row = None;
                    reasoning_row = None;
                    reasoning_draft.clear();
                    tool_rows.clear();
                    chat_rows.clear();
                    render_chat(chat_rows);
                    on_ui(|ui| ui.set_session_id("".into()));
                    status("New chat ready");
                }
                Some(Command::Send(text)) => {
                    if text.trim().is_empty() { continue; }
                    chat_rows.push(chat_row("user", "You", text.trim()));
                    render_chat(chat_rows);
                    if let Some(id) = &session_id {
                        let request_id = send_rpc(&mut sink, &mut next_id, "prompt.submit", json!({"session_id": id, "profile": client_state.session_profile, "text": text})).await?;
                        pending_prompts.insert(request_id);
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
                    chat_rows.clear();
                    draft.clear();
                    streaming_row = None;
                    reasoning_row = None;
                    reasoning_draft.clear();
                    tool_rows.clear();
                    render_chat(chat_rows);
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
                        chat_rows.push(chat_row("user", "Steer", &text));
                        render_chat(chat_rows);
                        status("Steering Hermes…");
                    }
                }
                Some(Command::LoadResource(resource)) => {
                    let (method, params) = match resource.as_str() {
                        "jobs" => ("cron.manage", json!({"action":"list","include_disabled":true,"profile":client_state.selected_profile})),
                        "skills" => ("skills.manage", json!({"action":"list","profile":client_state.selected_profile})),
                        "tools" => ("tools.list", json!({"session_id":session_id})),
                        "mcp" => ("mcp.servers.list", json!({"profile":client_state.selected_profile})),
                        _ => continue,
                    };
                    let id = send_rpc(&mut sink, &mut next_id, method, params).await?;
                    resource_requests.insert(id, resource.clone());
                    on_ui(move |ui| ui.set_resource_content("Loading…".into()));
                }
                Some(Command::ManageJob { action, name, schedule, prompt }) => {
                    if !matches!(action.as_str(), "add"|"pause"|"resume"|"remove") { continue; }
                    if name.trim().is_empty() || (action == "add" && (schedule.trim().is_empty() || prompt.trim().is_empty())) {
                        status("Enter a job name, plus schedule and prompt when adding.");
                        continue;
                    }
                    let id = send_rpc(&mut sink, &mut next_id, "cron.manage", json!({"action":action,"name":name.trim(),"schedule":schedule.trim(),"prompt":prompt.trim(),"profile":client_state.selected_profile})).await?;
                    job_mutation_ids.insert(id);
                    status("Updating job…");
                }
                Some(Command::TitleChat(title)) => {
                    if title.trim().is_empty() { status("Enter a chat title first"); continue; }
                    if let Some(id) = &session_id {
                        title_id = Some(send_rpc(&mut sink, &mut next_id, "session.title", json!({"session_id":id,"profile":client_state.session_profile,"title":title.trim()})).await?);
                    }
                }
                Some(Command::ArchiveChat) => {
                    if let Some(id) = &session_id {
                        archive_id = Some(send_rpc(&mut sink, &mut next_id, "session.archive", json!({"session_id":id,"profile":client_state.session_profile,"archived":true})).await?);
                    }
                }
                Some(Command::BranchChat(name)) => {
                    if let Some(id) = &session_id {
                        branch_id = Some(send_rpc(&mut sink, &mut next_id, "session.branch", json!({"session_id":id,"profile":client_state.session_profile,"name":if name.trim().is_empty() { None } else { Some(name.trim()) }})).await?);
                        status("Creating branch…");
                    }
                }
                Some(Command::SkillAction { action, query }) => {
                    if !matches!(action.as_str(), "search"|"browse"|"inspect"|"install") { continue; }
                    if action != "browse" && query.trim().is_empty() { status("Enter a skill query or identifier"); continue; }
                    let id = send_rpc(&mut sink, &mut next_id, "skills.manage", json!({"action":action,"query":query.trim(),"profile":client_state.selected_profile})).await?;
                    if action == "install" { skill_install_ids.insert(id); } else { resource_requests.insert(id, format!("skills:{action}")); }
                    on_ui(|ui| ui.set_resource_content("Loading skills…".into()));
                }
                Some(Command::ToolAction { action, name }) => {
                    if !matches!(action.as_str(), "enable"|"disable") || name.trim().is_empty() { status("Enter a toolset name"); continue; }
                    let id = send_rpc(&mut sink, &mut next_id, "tools.configure", json!({"action":action,"names":[name.trim()],"session_id":session_id,"profile":client_state.selected_profile})).await?;
                    tool_action_ids.insert(id);
                    status("Updating tools…");
                }
                Some(Command::McpTest(name)) => {
                    if name.trim().is_empty() { status("Enter an MCP server name"); continue; }
                    let id = send_rpc(&mut sink, &mut next_id, "mcp.servers.test", json!({"name":name.trim(),"profile":client_state.selected_profile})).await?;
                    resource_requests.insert(id, "mcp:test".to_owned());
                    on_ui(|ui| ui.set_resource_content("Testing MCP server…".into()));
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
                                    "tool.start" => {
                                        let name = payload.get("name").and_then(Value::as_str).unwrap_or("Tool");
                                        status(format!("Using {name}…"));
                                        let detail = payload.get("context").or_else(|| payload.get("preview")).and_then(Value::as_str).unwrap_or_default();
                                        let body = if detail.is_empty() { "Running…".to_owned() } else { format!("Running…\n{}", short_text(detail, 400)) };
                                        chat_rows.push(chat_row("tool", name, body));
                                        if let Some(id) = payload.get("tool_id").and_then(Value::as_str) {
                                            tool_rows.insert(id.to_owned(), chat_rows.len() - 1);
                                        }
                                        render_chat(chat_rows);
                                    }
                                    "tool.complete" => {
                                        status("Hermes is working…");
                                        let id = payload.get("tool_id").and_then(Value::as_str);
                                        let index = match id {
                                            Some(id) => tool_rows.remove(id),
                                            None => chat_rows.iter().rposition(|row| row.kind.as_str() == "tool" && row.body.as_str().starts_with("Running…")),
                                        };
                                        let body = tool_summary(&payload);
                                        if let Some(row) = index.and_then(|index| chat_rows.get_mut(index)) {
                                            row.body = body.into();
                                        } else {
                                            let name = payload.get("name").and_then(Value::as_str).unwrap_or("Tool");
                                            chat_rows.push(chat_row("tool", name, body));
                                        }
                                        render_chat(chat_rows);
                                    }
                                    "reasoning.delta" | "reasoning.available" => {
                                        if let Some(chunk) = payload.get("text").and_then(Value::as_str) {
                                            reasoning_draft.push_str(chunk);
                                            if reasoning_row.is_none() {
                                                chat_rows.push(chat_row("reasoning", "Reasoning", &reasoning_draft));
                                                reasoning_row = Some(chat_rows.len() - 1);
                                            } else if let Some(row) = reasoning_row.and_then(|index| chat_rows.get_mut(index)) {
                                                row.body = reasoning_draft.clone().into();
                                            }
                                            render_chat(chat_rows);
                                        }
                                    }
                                    "message.delta" => {
                                        if let Some(chunk) = payload.get("text").or_else(|| payload.get("delta")).and_then(Value::as_str) {
                                            draft.push_str(chunk);
                                            if streaming_row.is_none() {
                                                chat_rows.push(chat_row("assistant", "Goblin", &draft));
                                                streaming_row = Some(chat_rows.len() - 1);
                                            } else if let Some(row) = streaming_row.and_then(|index| chat_rows.get_mut(index)) {
                                                row.body = draft.clone().into();
                                            }
                                            render_chat(chat_rows);
                                        }
                                    }
                                    "message.start" => {
                                        on_ui(|ui| ui.set_busy(true));
                                        reasoning_row = None;
                                        reasoning_draft.clear();
                                        if streaming_row.is_none() {
                                            chat_rows.push(chat_row("assistant", "Goblin", "Thinking…"));
                                            streaming_row = Some(chat_rows.len() - 1);
                                            render_chat(chat_rows);
                                        }
                                    }
                                    "message.complete" => {
                                        on_ui(|ui| ui.set_busy(false));
                                        if let Some(full) = payload.get("text").and_then(Value::as_str) {
                                            draft = full.to_owned();
                                        }
                                        if !draft.is_empty() {
                                            if let Some(row) = streaming_row.and_then(|index| chat_rows.get_mut(index)) {
                                                row.body = draft.clone().into();
                                            } else {
                                                chat_rows.push(chat_row("assistant", "Goblin", &draft));
                                            }
                                            draft.clear();
                                            render_chat(chat_rows);
                                        }
                                        streaming_row = None;
                                        reasoning_row = None;
                                        reasoning_draft.clear();
                                        status("Ready");
                                    }
                                    "message.interim" => {
                                        if payload.get("already_streamed").and_then(Value::as_bool) != Some(true) {
                                            if let Some(text) = payload.get("text").and_then(Value::as_str) {
                                                draft.push_str(text);
                                                draft.push_str("\n\n");
                                                if let Some(row) = streaming_row.and_then(|index| chat_rows.get_mut(index)) {
                                                    row.body = draft.clone().into();
                                                } else {
                                                    chat_rows.push(chat_row("assistant", "Goblin", &draft));
                                                    streaming_row = Some(chat_rows.len() - 1);
                                                }
                                                render_chat(chat_rows);
                                            }
                                        }
                                    }
                                    "notice" => {
                                        if let Some(message) = payload.get("message").and_then(Value::as_str).filter(|text| !text.trim().is_empty()) {
                                            chat_rows.push(chat_row("notice", "Notice", message));
                                            render_chat(chat_rows);
                                        }
                                    }
                                    "error" => {
                                        let message = payload.get("message").and_then(Value::as_str).unwrap_or("unknown");
                                        status(format!("Hermes error: {message}"));
                                        chat_rows.push(chat_row("notice", "Hermes error", message));
                                        render_chat(chat_rows);
                                    }
                                    _ => {}
                                }
                            }
                            Ok(RpcFrame::Response { id: response_id, result }) => match result {
                                Ok(value) => {
                                    pending_prompts.remove(&response_id);
                                    if let Some(resource) = resource_requests.remove(&response_id) {
                                        let content = format_resource(&resource, &value);
                                        on_ui(move |ui| ui.set_resource_content(content.into()));
                                        continue;
                                    }
                                    if job_mutation_ids.remove(&response_id) {
                                        if let Some(error) = value.get("error").and_then(Value::as_str) {
                                            status(format!("Job update failed: {error}"));
                                        } else {
                                            status("Job updated");
                                            let id = send_rpc(&mut sink, &mut next_id, "cron.manage", json!({"action":"list","include_disabled":true,"profile":client_state.selected_profile})).await?;
                                            resource_requests.insert(id, "jobs".to_owned());
                                        }
                                        continue;
                                    }
                                    if skill_install_ids.remove(&response_id) {
                                        let installed = value.get("installed").and_then(Value::as_bool) == Some(true);
                                        status(if installed { "Skill installed" } else { "Skill installation did not complete" });
                                        let id = send_rpc(&mut sink, &mut next_id, "skills.manage", json!({"action":"list","profile":client_state.selected_profile})).await?;
                                        resource_requests.insert(id, "skills".to_owned());
                                        continue;
                                    }
                                    if tool_action_ids.remove(&response_id) {
                                        let unknown = value.get("unknown").and_then(Value::as_array).is_some_and(|items| !items.is_empty());
                                        let missing = value.get("missing_servers").and_then(Value::as_array).is_some_and(|items| !items.is_empty());
                                        status(if unknown || missing { "Some toolsets were not found. Check their names." } else { "Toolset updated" });
                                        let id = send_rpc(&mut sink, &mut next_id, "tools.list", json!({"session_id":session_id})).await?;
                                        resource_requests.insert(id, "tools".to_owned());
                                        continue;
                                    }
                                    if title_id.as_deref() == Some(&response_id) { title_id = None; status("Chat renamed"); continue; }
                                    if archive_id.as_deref() == Some(&response_id) {
                                        archive_id = None;
                                        session_id = None;
                                        *restore_id = None;
                                        chat_rows.clear();
                                        streaming_row = None;
                                        reasoning_row = None;
                                        reasoning_draft.clear();
                                        tool_rows.clear();
                                        render_chat(chat_rows);
                                        on_ui(|ui| ui.set_session_id("".into()));
                                        status("Chat archived");
                                        continue;
                                    }
                                    if branch_id.as_deref() == Some(&response_id) {
                                        branch_id = None;
                                        if let Some(id) = value.get("stored_session_id").and_then(Value::as_str) {
                                            *restore_id = Some(id.to_owned());
                                            attach_id = Some(send_rpc(&mut sink, &mut next_id, "session.resume", json!({"session_id":id,"profile":client_state.session_profile,"source":"mobile-goblin","cols":80,"omit_messages":true})).await?);
                                        }
                                        status("Branch created");
                                        continue;
                                    }
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
                                                let request_id = send_rpc(&mut sink, &mut next_id, "prompt.submit", json!({"session_id": id, "profile": client_state.session_profile, "text": text, "queued": queued})).await?;
                                                pending_prompts.insert(request_id);
                                                on_ui(|ui| ui.set_busy(true));
                                                queued = true;
                                            }
                                        }
                                    }
                                }
                                Err(fault) => {
                                    resource_requests.remove(&response_id);
                                    job_mutation_ids.remove(&response_id);
                                    skill_install_ids.remove(&response_id);
                                    tool_action_ids.remove(&response_id);
                                    if attach_id.as_deref() == Some(&response_id) {
                                        attach_id = None;
                                        pending_send.clear();
                                        chat_rows.push(chat_row("notice", "Connection", "Chat could not be opened. Message was not sent."));
                                        render_chat(chat_rows);
                                    }
                                    if pending_prompts.remove(&response_id) {
                                        on_ui(|ui| ui.set_busy(false));
                                        chat_rows.push(chat_row("notice", "Hermes", "Message was not accepted by Hermes."));
                                        render_chat(chat_rows);
                                    }
                                    if model_set_id.as_deref() == Some(&response_id) { model_candidate = None; }
                                    status(format!("Hermes: {}", fault.message));
                                },
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
                Some(Ok(Message::Pong(_))) => { last_inbound = std::time::Instant::now(); },
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

fn history_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    return Some(text.to_owned());
                }
                let kind = part.get("type").and_then(Value::as_str).unwrap_or_default();
                if matches!(kind, "image" | "image_url" | "input_image") {
                    Some("[Image attachment]".to_owned())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn history_reasoning(row: &Value) -> String {
    let base = row
        .get("display_reasoning")
        .or_else(|| row.get("reasoning"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut parts = Vec::new();
    if !base.trim().is_empty() {
        parts.push(base.trim().to_owned());
    }
    if let Some(commentary) = row.get("display_commentary").and_then(Value::as_array) {
        for item in commentary.iter().filter_map(Value::as_str) {
            let item = item.trim();
            if !item.is_empty() && !parts.iter().any(|existing| existing.contains(item)) {
                parts.push(item.to_owned());
            }
        }
    }
    parts.join("\n\n")
}

fn short_text(value: &str, limit: usize) -> String {
    let mut chars = value.trim().chars();
    let preview: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

fn tool_summary(payload: &Value) -> String {
    let failed = payload
        .get("result")
        .and_then(|result| result.get("is_error").or_else(|| result.get("error")))
        .is_some_and(|value| {
            value.as_bool() == Some(true) || value.as_str().is_some_and(|text| !text.is_empty())
        });
    let summary = payload
        .get("summary")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .or_else(|| payload.get("result_text").and_then(Value::as_str))
        .unwrap_or(if failed { "Tool failed" } else { "Finished" });
    let prefix = if failed { "Failed: " } else { "" };
    format!("{prefix}{}", short_text(summary, 500))
}

fn present_sessions(page: &Value, field: &str) -> usize {
    let mut seen = HashSet::new();
    let rows: Vec<SessionRow> = page
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_owned();
            if !seen.insert(id.clone()) {
                return None;
            }
            let title = item
                .get("title")
                .and_then(Value::as_str)
                .filter(|title| !title.trim().is_empty())
                .or_else(|| item.get("preview").and_then(Value::as_str))
                .unwrap_or("Untitled chat")
                .lines()
                .next()
                .unwrap_or("Untitled chat");
            let title = if field == "results" {
                let snippet = item
                    .get("snippet")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .replace(">>>", "")
                    .replace("<<<", "")
                    .split_whitespace()
                    .take(14)
                    .collect::<Vec<_>>()
                    .join(" ");
                if snippet.is_empty() {
                    title.to_owned()
                } else {
                    format!("{title} · {snippet}")
                }
            } else {
                title.to_owned()
            };
            Some(SessionRow {
                id: id.into(),
                title: title.into(),
            })
        })
        .collect();
    let count = rows.len();
    on_ui(move |ui| ui.set_sessions(slint::ModelRc::new(slint::VecModel::from(rows))));
    count
}

fn format_resource(resource: &str, value: &Value) -> String {
    let mut lines = Vec::new();
    match resource {
        "jobs" => {
            for job in value
                .get("jobs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let name = job.get("name").and_then(Value::as_str).unwrap_or("Unnamed");
                let schedule = job.get("schedule").and_then(Value::as_str).unwrap_or("?");
                let state = if job.get("enabled").and_then(Value::as_bool) == Some(false) {
                    "paused"
                } else {
                    "enabled"
                };
                let preview = job
                    .get("prompt_preview")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let next = job.get("next_run_at").and_then(Value::as_str).unwrap_or("");
                lines.push(format!(
                    "{name} · {state}\n{schedule}\n{preview}\nNext: {next}"
                ));
            }
        }
        "skills" => {
            if let Some(categories) = value.get("skills").and_then(Value::as_object) {
                let mut keys: Vec<_> = categories.keys().collect();
                keys.sort();
                for key in keys {
                    let names = categories[key]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>();
                    lines.push(format!("{}\n{}", key, names.join(", ")));
                }
            }
        }
        "skills:search" | "skills:browse" => {
            let key = if resource == "skills:search" {
                "results"
            } else {
                "items"
            };
            for item in value
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let name = item
                    .get("name")
                    .or_else(|| item.get("slug"))
                    .and_then(Value::as_str)
                    .unwrap_or("Skill");
                let description = item
                    .get("description")
                    .or_else(|| item.get("summary"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                lines.push(format!("{name}\n{description}"));
            }
        }
        "skills:inspect" => {
            if let Some(info) = value.get("info") {
                lines.push(
                    serde_json::to_string_pretty(info)
                        .unwrap_or_else(|_| "Cannot show skill".to_owned()),
                );
            }
        }
        "tools" => {
            for toolset in value
                .get("toolsets")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let name = toolset
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("Toolset");
                let count = toolset
                    .get("tool_count")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let state = if toolset.get("enabled").and_then(Value::as_bool) == Some(true) {
                    "on"
                } else {
                    "off"
                };
                let description = toolset
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                lines.push(format!("{name} · {count} tools · {state}\n{description}"));
            }
        }
        "mcp" => {
            for server in value
                .get("servers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let name = server
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("Server");
                let transport = server
                    .get("transport")
                    .and_then(Value::as_str)
                    .unwrap_or("?");
                let state = if server.get("enabled").and_then(Value::as_bool) == Some(true) {
                    "on"
                } else {
                    "off"
                };
                lines.push(format!("{name} · {transport} · {state}"));
            }
        }
        "mcp:test" => {
            lines.push(
                serde_json::to_string_pretty(value)
                    .unwrap_or_else(|_| "Cannot show test result".to_owned()),
            );
        }
        _ => {}
    }
    if lines.is_empty() {
        "No items found".to_owned()
    } else {
        lines.join("\n\n")
    }
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

#[cfg(test)]
mod tests {
    use super::{history_reasoning, history_text, short_text, tool_summary};
    use serde_json::json;

    #[test]
    fn history_keeps_text_and_marks_image_parts() {
        let content = json!([
            {"type":"text","text":"What is in this picture?"},
            {"type":"image_url","image_url":{"url":"data:image/png;base64,..."}},
            {"type":"text","text":"Please be concise."}
        ]);
        assert_eq!(
            history_text(Some(&content)),
            "What is in this picture?\n[Image attachment]\nPlease be concise."
        );
    }

    #[test]
    fn history_prefers_display_reasoning_without_repeating_commentary() {
        let row = json!({
            "reasoning": "legacy",
            "display_reasoning": "Plan\nReview",
            "display_commentary": ["Review", "Done"]
        });
        assert_eq!(history_reasoning(&row), "Plan\nReview\n\nDone");
    }

    #[test]
    fn tool_summary_uses_gateway_summary_and_bounds_output() {
        assert_eq!(
            tool_summary(&json!({"summary":"Finished 4 files"})),
            "Finished 4 files"
        );
        assert_eq!(
            tool_summary(&json!({"result":{"error":"no access"},"summary":"Denied"})),
            "Failed: Denied"
        );
        assert_eq!(short_text("ééé", 2), "éé…");
    }
}
