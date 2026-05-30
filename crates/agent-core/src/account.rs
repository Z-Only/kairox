use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountState {
    pub login_required: bool,
    pub settings_sync_enabled: bool,
    pub subscription_plan: Option<String>,
}

#[async_trait]
pub trait AccountService: Send + Sync {
    async fn current_account(&self) -> crate::Result<AccountState>;
}

#[derive(Debug, Clone)]
pub struct LocalNoAccountService;

#[async_trait]
impl AccountService for LocalNoAccountService {
    async fn current_account(&self) -> crate::Result<AccountState> {
        Ok(AccountState {
            login_required: false,
            settings_sync_enabled: false,
            subscription_plan: None,
        })
    }
}

#[cfg(test)]
#[path = "account_tests.rs"]
mod tests;
