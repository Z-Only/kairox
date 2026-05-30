use super::*;

#[tokio::test]
async fn local_no_account_preserves_full_local_use() {
    let service = LocalNoAccountService;
    let state = service.current_account().await.unwrap();

    assert!(!state.login_required);
    assert!(!state.settings_sync_enabled);
    assert_eq!(state.subscription_plan, None);
}
