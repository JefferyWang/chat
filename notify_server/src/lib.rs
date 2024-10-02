mod config;
mod error;
mod notify;
mod sse;

use std::{ops::Deref, sync::Arc};

use anyhow::Result;
use axum::{
    http::Method,
    middleware::from_fn_with_state,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use chat_core::{
    middlewares::{verify_token, TokenVerify},
    DecodingKey, User,
};
pub use config::AppConfig;
use dashmap::DashMap;
pub use error::AppError;
pub use notify::*;
use sse::sse_handler;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

pub type UserMap = Arc<DashMap<u64, broadcast::Sender<Arc<AppEvent>>>>;

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

pub struct AppStateInner {
    pub config: AppConfig,
    users: UserMap,
    dk: DecodingKey,
}

const INDEX_HTML: &str = include_str!("../index.html");

pub async fn get_router(config: AppConfig) -> Result<Router> {
    let state = AppState::new(config);
    setup_pg_listener(state.clone()).await?;

    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::PUT,
        ])
        .allow_headers(Any)
        .allow_origin(Any);

    let app = Router::new()
        .route("/events", get(sse_handler))
        .layer(from_fn_with_state(state.clone(), verify_token::<AppState>))
        .route("/", get(index_handler))
        .with_state(state)
        .layer(cors);
    Ok(app)
}

async fn index_handler() -> impl IntoResponse {
    Html(INDEX_HTML)
}

impl TokenVerify for AppState {
    type Error = AppError;

    fn verify(&self, token: &str) -> Result<User, Self::Error> {
        Ok(self.0.dk.verify(token)?)
    }
}

impl Deref for AppState {
    type Target = AppStateInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let dk = DecodingKey::load(&config.auth.pk).expect("Failed to load public key");
        let users = Arc::new(DashMap::new());
        Self(Arc::new(AppStateInner { config, dk, users }))
    }
}
