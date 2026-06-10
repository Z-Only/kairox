use super::*;

#[test]
fn priority_returns_discriminant_value() {
    assert_eq!(ConfigScope::Builtin.priority(), 0);
    assert_eq!(ConfigScope::User.priority(), 1);
    assert_eq!(ConfigScope::Project.priority(), 2);
    assert_eq!(ConfigScope::Local.priority(), 3);
}

#[test]
fn label_returns_lowercase_name() {
    assert_eq!(ConfigScope::Builtin.label(), "builtin");
    assert_eq!(ConfigScope::User.label(), "user");
    assert_eq!(ConfigScope::Project.label(), "project");
    assert_eq!(ConfigScope::Local.label(), "local");
}

#[test]
fn display_matches_label() {
    assert_eq!(format!("{}", ConfigScope::Builtin), "builtin");
    assert_eq!(format!("{}", ConfigScope::Local), "local");
}

#[test]
fn partial_ord_follows_priority() {
    assert!(ConfigScope::Builtin < ConfigScope::User);
    assert!(ConfigScope::User < ConfigScope::Project);
    assert!(ConfigScope::Project < ConfigScope::Local);
}

#[test]
fn serde_round_trip() {
    let scope = ConfigScope::Project;
    let json = serde_json::to_string(&scope).unwrap();
    let deserialized: ConfigScope = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, scope);
}

#[test]
fn serde_deserializes_from_string() {
    let scope: ConfigScope = serde_json::from_str("\"Project\"").unwrap();
    assert_eq!(scope, ConfigScope::Project);
}
