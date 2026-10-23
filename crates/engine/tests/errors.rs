//! Error codes and the contract version (spec 004: contract 1.1.0 adds NOT_FOUND).

use engine::{ErrorCode, CONTRACT_VERSION};

#[test]
fn contract_is_1_1_0() {
    assert_eq!(CONTRACT_VERSION, "1.1.0");
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
