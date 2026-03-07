use std::fs;
use std::path::Path;
use viper_domain::StrategyDecisionEvent;

fn load_fixture(name: &str) -> StrategyDecisionEvent {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let path = base.join(name);
    let raw = fs::read_to_string(path).expect("fixture exists");
    serde_json::from_str::<StrategyDecisionEvent>(&raw).expect("fixture deserializes")
}

#[test]
fn fixture_enter_long_validates() {
    let event = load_fixture("enter_long.json");
    assert_eq!(event.decision.action, "ENTER_LONG");
    event.validate().expect("must validate");
}

#[test]
fn fixture_enter_short_validates() {
    let event = load_fixture("enter_short.json");
    assert_eq!(event.decision.action, "ENTER_SHORT");
    event.validate().expect("must validate");
}

#[test]
fn fixture_hold_validates() {
    let event = load_fixture("hold.json");
    assert_eq!(event.decision.action, "HOLD");
    event.validate().expect("must validate");
}

#[test]
fn fixture_close_long_validates() {
    let event = load_fixture("close_long.json");
    assert_eq!(event.decision.action, "CLOSE_LONG");
    event.validate().expect("must validate");
}

#[test]
fn fixture_close_short_validates() {
    let event = load_fixture("close_short.json");
    assert_eq!(event.decision.action, "CLOSE_SHORT");
    event.validate().expect("must validate");
}
