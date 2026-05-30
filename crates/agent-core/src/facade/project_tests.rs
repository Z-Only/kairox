use super::*;

#[test]
fn project_id_round_trips_from_string() {
    let project_id = ProjectId::new();
    let encoded = project_id.to_string();

    let decoded = ProjectId::from_string(encoded.clone());

    assert_eq!(decoded.to_string(), encoded);
}

#[test]
fn project_visibility_serializes_as_snake_case() {
    let value = serde_json::to_value(ProjectSessionVisibility::DraftHidden).unwrap();

    assert_eq!(value, serde_json::json!("draft_hidden"));
}
