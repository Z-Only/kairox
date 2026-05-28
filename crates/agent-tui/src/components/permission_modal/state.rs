use crate::components::PermissionRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PermissionHistoryEntry {
    pub request: PermissionRequest,
    pub approved: bool,
}

pub struct PermissionModal {
    pub(super) focused: bool,
    pub request: Option<PermissionRequest>,
    pub(super) pending_requests: Vec<PermissionRequest>,
    pub(super) history: Vec<PermissionHistoryEntry>,
}

impl Default for PermissionModal {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionModal {
    pub fn new() -> Self {
        Self {
            focused: false,
            request: None,
            pending_requests: Vec::new(),
            history: Vec::new(),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.request.is_some()
    }

    pub(super) fn enqueue_request(&mut self, request: PermissionRequest) {
        if self
            .pending_requests
            .iter()
            .any(|pending| pending.request_id == request.request_id)
            || self
                .request
                .as_ref()
                .is_some_and(|pending| pending.request_id == request.request_id)
        {
            return;
        }

        self.pending_requests.push(request);
        if self.request.is_none() {
            self.sync_active_request();
        }
    }

    pub(super) fn resolve_active_request(
        &mut self,
        approved: bool,
    ) -> Option<PermissionRequest> {
        let request_id = self.request.as_ref()?.request_id.clone();
        self.resolve_request(&request_id, approved)
    }

    pub(super) fn resolve_request(
        &mut self,
        request_id: &str,
        approved: bool,
    ) -> Option<PermissionRequest> {
        let resolved = if let Some(index) = self
            .pending_requests
            .iter()
            .position(|pending| pending.request_id == request_id)
        {
            Some(self.pending_requests.remove(index))
        } else if self
            .request
            .as_ref()
            .is_some_and(|pending| pending.request_id == request_id)
        {
            self.request.take()
        } else {
            None
        };

        if let Some(request) = resolved.as_ref() {
            self.push_history(request.clone(), approved);
        }
        self.sync_active_request();
        resolved
    }

    fn sync_active_request(&mut self) {
        self.request = self.pending_requests.first().cloned();
    }

    fn push_history(&mut self, request: PermissionRequest, approved: bool) {
        self.history
            .push(PermissionHistoryEntry { request, approved });
        const MAX_HISTORY: usize = 6;
        if self.history.len() > MAX_HISTORY {
            let excess = self.history.len() - MAX_HISTORY;
            self.history.drain(0..excess);
        }
    }
}
