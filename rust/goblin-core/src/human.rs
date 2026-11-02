//! Human-input requests sent by Hermes over the dashboard WebSocket.
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq)]
pub enum HumanRequest {
    Approval {
        id: String,
        session_id: Option<String>,
        description: String,
        command: String,
        choices: Vec<String>,
    },
    Clarify {
        id: String,
        session_id: Option<String>,
        questions: Vec<Question>,
    },
    Secret {
        id: String,
        session_id: Option<String>,
        prompt: String,
        masked: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    pub qid: Option<String>,
    pub text: String,
    pub choices: Vec<String>,
    pub multi_select: bool,
}

impl HumanRequest {
    pub fn parse(id: String, method: &str, params: &Value) -> Option<Self> {
        let session_id = params
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        match method {
            "approval" => {
                let offered = params
                    .get("choices")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .filter(|s| {
                                matches!(s.as_str(), "once" | "session" | "always" | "deny")
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let mut choices = if offered.is_empty() {
                    let mut fallback = vec!["once".to_owned()];
                    if params.get("smart_denied").and_then(Value::as_bool) != Some(true)
                        && params.get("allow_session").and_then(Value::as_bool) != Some(false)
                    {
                        fallback.push("session".to_owned());
                        if params.get("allow_permanent").and_then(Value::as_bool) != Some(false) {
                            fallback.push("always".to_owned());
                        }
                    }
                    fallback.push("deny".to_owned());
                    fallback
                } else {
                    offered
                };
                if !choices.iter().any(|s| s == "once") {
                    choices.insert(0, "once".to_owned());
                }
                if !choices.iter().any(|s| s == "deny") {
                    choices.push("deny".to_owned());
                }
                Some(Self::Approval {
                    id,
                    session_id,
                    description: string(params, "description"),
                    command: string(params, "command"),
                    choices,
                })
            }
            "clarify" => {
                let batch = params
                    .get("questions")
                    .and_then(Value::as_array)
                    .map(|list| {
                        list.iter()
                            .filter_map(|q| {
                                Some(Question {
                                    qid: Some(q.get("qid")?.as_str()?.to_owned()),
                                    text: q.get("question")?.as_str()?.to_owned(),
                                    choices: strings(q, "choices"),
                                    multi_select: q.get("multi_select").and_then(Value::as_bool)
                                        == Some(true),
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let questions = if batch.is_empty() {
                    vec![Question {
                        qid: None,
                        text: params.get("question")?.as_str()?.to_owned(),
                        choices: strings(params, "choices"),
                        multi_select: params.get("multi_select").and_then(Value::as_bool)
                            == Some(true),
                    }]
                } else {
                    batch
                };
                Some(Self::Clarify {
                    id,
                    session_id,
                    questions,
                })
            }
            "sudo" | "secret" | "vault.unlock_prompt" | "vault.code" => {
                let prompt = match method {
                    "sudo" => format!("Sudo password for {}", string(params, "command")),
                    "secret" => params
                        .get("prompt")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("Value for {}", string(params, "env_var"))),
                    "vault.unlock_prompt" => format!(
                        "Unlock {}",
                        params
                            .get("display_name")
                            .or_else(|| params.get("backend"))
                            .and_then(Value::as_str)
                            .unwrap_or("password manager")
                    ),
                    _ => format!(
                        "Sign-in code for {}",
                        params
                            .get("site")
                            .and_then(Value::as_str)
                            .unwrap_or("this site")
                    ),
                };
                Some(Self::Secret {
                    id,
                    session_id,
                    prompt,
                    masked: method != "vault.code",
                })
            }
            _ => None,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Approval { id, .. } | Self::Clarify { id, .. } | Self::Secret { id, .. } => id,
        }
    }
    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::Approval { session_id, .. }
            | Self::Clarify { session_id, .. }
            | Self::Secret { session_id, .. } => session_id.as_deref(),
        }
    }
}

pub fn approval_result(choice: &str, offered: &[String]) -> Option<Value> {
    offered
        .iter()
        .any(|s| s == choice)
        .then(|| json!({"choice":choice}))
}

pub fn clarify_result(questions: &[Question], answers: &[String]) -> Option<Value> {
    if questions.len() != answers.len() {
        return None;
    }
    if questions.iter().all(|q| q.qid.is_some()) {
        let mut result = serde_json::Map::new();
        for (q, answer) in questions.iter().zip(answers) {
            result.insert(
                q.qid.as_ref()?.clone(),
                Value::String(normalize_answer(q, answer)),
            );
        }
        Some(json!({"answers":result}))
    } else if questions.len() == 1 {
        Some(json!({"answer":normalize_answer(&questions[0], &answers[0])}))
    } else {
        None
    }
}

fn normalize_answer(question: &Question, answer: &str) -> String {
    if !question.multi_select || answer.trim().is_empty() {
        return answer.to_owned();
    }
    if serde_json::from_str::<Vec<String>>(answer).is_ok() {
        return answer.to_owned();
    }
    let choices: Vec<&str> = answer
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    serde_json::to_string(&choices).unwrap_or_else(|_| "[]".to_owned())
}

fn string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
fn strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}
