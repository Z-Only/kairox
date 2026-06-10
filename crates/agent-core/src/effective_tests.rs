use super::*;
use crate::config_scope::ConfigScope;

#[test]
fn new_with_builtin_source_is_not_writable_or_deletable() {
    let item = EffectiveItem::new("value".to_string(), ConfigScope::Builtin);
    assert!(item.enabled);
    assert!(!item.writable);
    assert!(!item.deletable);
    assert!(item.overrides.is_none());
    assert!(item.disabled_by.is_none());
}

#[test]
fn new_with_user_source_is_writable_and_deletable() {
    let item = EffectiveItem::new(42, ConfigScope::User);
    assert!(item.enabled);
    assert!(item.writable);
    assert!(item.deletable);
    assert_eq!(item.source, ConfigScope::User);
}

#[test]
fn new_with_project_source_is_writable_and_deletable() {
    let item = EffectiveItem::new("x".to_string(), ConfigScope::Project);
    assert!(item.writable);
    assert!(item.deletable);
}

#[test]
fn new_with_local_source_is_writable_and_deletable() {
    let item = EffectiveItem::new(true, ConfigScope::Local);
    assert!(item.writable);
    assert!(item.deletable);
}

#[test]
fn with_disabled_sets_enabled_false_and_disabled_by() {
    let item = EffectiveItem::new("val".to_string(), ConfigScope::User)
        .with_disabled(ConfigScope::Project);
    assert!(!item.enabled);
    assert_eq!(item.disabled_by, Some(ConfigScope::Project));
}

#[test]
fn with_override_sets_overrides_field() {
    let item = EffectiveItem::new("val".to_string(), ConfigScope::Builtin)
        .with_override(ConfigScope::User);
    assert_eq!(item.overrides, Some(ConfigScope::User));
    assert!(item.enabled); // override doesn't disable
}

#[test]
fn chained_with_disabled_and_override() {
    let item = EffectiveItem::new(99, ConfigScope::User)
        .with_override(ConfigScope::Project)
        .with_disabled(ConfigScope::Local);
    assert!(!item.enabled);
    assert_eq!(item.overrides, Some(ConfigScope::Project));
    assert_eq!(item.disabled_by, Some(ConfigScope::Local));
}

#[test]
fn serde_round_trip() {
    let item = EffectiveItem::new("hello".to_string(), ConfigScope::Project)
        .with_override(ConfigScope::Local);
    let json = serde_json::to_string(&item).unwrap();
    let restored: EffectiveItem<String> = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.value, "hello");
    assert_eq!(restored.source, ConfigScope::Project);
    assert_eq!(restored.overrides, Some(ConfigScope::Local));
    assert!(restored.enabled);
}
