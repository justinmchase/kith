use kith::host::{evaluate, match_message};
use serde_json::json;
use std::collections::HashMap;

#[test]
// req:uffda-host-001
fn evaluate_add_20_22_yields_42() {
    let result = evaluate(
        &json!({
            "kind": "invocation",
            "expression": {"kind": "reference", "name": "add"},
            "args": [
                {"kind": "number", "value": 20},
                {"kind": "number", "value": 22}
            ]
        }),
        &HashMap::new(),
    )
    .expect("evaluate");
    assert_eq!(result, json!(42.0));
}

#[test]
// req:uffda-host-002
fn object_pattern_matches_ping_and_rejects_pong() {
    let pattern = json!({
        "kind": "over",
        "keys": {
            "kind": {
                "kind": "equal",
                "value": {"kind": "value.literal", "value": "Ping"}
            },
            "n": {
                "kind": "variable",
                "name": "n",
                "pattern": {"kind": "any"}
            }
        },
        "rest": null
    });
    let matched = match_message(&pattern, &json!({"kind": "Ping", "n": 7})).expect("match");
    let bindings = matched.expect("Ping should match");
    assert_eq!(bindings.get("n"), Some(&json!(7.0)));
    let missed = match_message(&pattern, &json!({"kind": "Pong", "n": 7})).expect("match");
    assert!(missed.is_none());
}

#[test]
// req:workflows-003
fn reduce_step_adds_one_to_the_accumulator() {
    let pattern = json!({
        "kind": "over",
        "keys": {
            "total": {
                "kind": "variable",
                "name": "total",
                "pattern": {"kind": "any"}
            }
        }
    });
    let bindings = match_message(&pattern, &json!({"total": 1.0}))
        .expect("match")
        .expect("accumulator should match");
    assert_eq!(
        evaluate(
            &json!({
                "kind": "invocation",
                "expression": {"kind": "reference", "name": "add"},
                "args": [
                    {"kind": "reference", "name": "total"},
                    {"kind": "number", "value": 1}
                ]
            }),
            &bindings,
        )
        .expect("add"),
        json!(2.0)
    );
}
