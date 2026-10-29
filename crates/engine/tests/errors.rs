//! Error codes and the contract version (spec 004: 1.1.0 adds NOT_FOUND; spec 007: 1.2.0 adds
//! BUSY; spec 008: 1.3.0 adds the updateGraph task; spec 009: 1.4.0 adds query and its codes).

use engine::{ErrorCode, CONTRACT_VERSION};

#[test]
fn contract_is_1_4_0() {
    assert_eq!(CONTRACT_VERSION, "1.4.0");
}

#[test]
fn busy_is_a_contract_error_code() {
    assert_eq!(ErrorCode::Busy.as_str(), "BUSY");
    assert!(ErrorCode::ALL.contains(&ErrorCode::Busy));
    assert_eq!(
        serde_json::to_value(ErrorCode::Busy).unwrap(),
        serde_json::json!("BUSY")
    );
}

#[test]
fn not_found_is_a_contract_error_code() {
    assert_eq!(ErrorCode::NotFound.as_str(), "NOT_FOUND");
    assert!(ErrorCode::ALL.contains(&ErrorCode::NotFound));
    assert_eq!(
        serde_json::to_value(ErrorCode::NotFound).unwrap(),
        serde_json::json!("NOT_FOUND")
    );
}
