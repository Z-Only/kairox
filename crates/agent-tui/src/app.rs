use agent_core::projection::SessionProjection;

#[derive(Debug, Default)]
pub struct TuiApp {
    pub projection: SessionProjection,
    pub status: String,
}

impl TuiApp {
    pub fn set_projection(&mut self, projection: SessionProjection) {
        self.projection = projection;
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }
}
