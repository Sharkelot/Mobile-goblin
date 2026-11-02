use goblin_core::human::{HumanRequest, approval_result, clarify_result};
use serde_json::json;

#[test]
fn approval_only_accepts_offered_choices() {
    let request = HumanRequest::parse(
        "srq-1".into(),
        "approval",
        &json!({
            "session_id":"s1", "command":"rm -rf /tmp/example", "choices":["once","deny"]
        }),
    )
    .unwrap();
    let HumanRequest::Approval { choices, .. } = request else {
        panic!("approval expected")
    };
    assert_eq!(
        approval_result("once", &choices),
        Some(json!({"choice":"once"}))
    );
    assert_eq!(approval_result("always", &choices), None);
}

#[test]
fn batch_clarification_retains_question_ids() {
    let request = HumanRequest::parse(
        "srq-2".into(),
        "clarify",
        &json!({
            "questions":[{"qid":"a","question":"Color?"},{"qid":"b","question":"Size?"}]
        }),
    )
    .unwrap();
    let HumanRequest::Clarify { questions, .. } = request else {
        panic!("clarify expected")
    };
    assert_eq!(
        clarify_result(&questions, &["green".into(), "small".into()]),
        Some(json!({"answers":{"a":"green","b":"small"}}))
    );
    assert_eq!(clarify_result(&questions, &["green".into()]), None);
}

#[test]
fn multi_select_answers_use_gateway_array_string() {
    let request = HumanRequest::parse(
        "srq-3".into(),
        "clarify",
        &json!({
            "question":"Pick colors", "multi_select":true, "choices":["green","blue"]
        }),
    )
    .unwrap();
    let HumanRequest::Clarify { questions, .. } = request else {
        panic!("clarify expected")
    };
    assert_eq!(
        clarify_result(&questions, &["green, blue".into()]),
        Some(json!({"answer":"[\"green\",\"blue\"]"}))
    );
}
