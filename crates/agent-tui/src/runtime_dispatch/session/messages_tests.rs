use super::*;

#[test]
fn goal_command_prepares_model_content_and_display_content() {
    let (model_content, display_content) =
        prepare_goal_message_for_dispatch(":goal fix flaky tests".to_string());

    assert!(model_content.contains("# Goal"));
    assert!(model_content.contains("fix flaky tests"));
    assert_eq!(display_content.as_deref(), Some(":goal fix flaky tests"));
}

#[test]
fn malformed_goal_command_is_left_unchanged() {
    let (model_content, display_content) = prepare_goal_message_for_dispatch(":goal ".to_string());

    assert_eq!(model_content, ":goal ");
    assert_eq!(display_content, None);
}
