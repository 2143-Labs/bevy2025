//! Native (non-web) auth backend: authserver URL from `--auth-url`, HTTP on the tokio runtime
//! that `main` already runs.

use std::sync::Arc;

use bevy::prelude::*;

use super::{AuthBaseUrl, http::AuthRuntime};

pub struct NativeAuthPlugin {
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub auth_url: String,
}

impl Plugin for NativeAuthPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AuthRuntime::new(self.runtime.clone()))
            .insert_resource(AuthBaseUrl(self.auth_url.trim_end_matches('/').to_string()));
    }
}
