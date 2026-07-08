mod achievements;
mod auth;
mod friends;
mod health;
mod identities;
mod users;

use axum::Router;

use crate::db::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(auth::router())
        .merge(friends::router())
        .merge(users::router())
        .merge(identities::router())
        .merge(achievements::router())
}
