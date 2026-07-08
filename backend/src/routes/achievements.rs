use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

use crate::achievements::try_unlock;
use crate::auth::extract_user_id;
use crate::db::AppState;
use crate::error::ApiError;
use crate::routes::users::ensure_can_view_user;

#[derive(Serialize)]
pub struct AchievementRow {
    pub code: String,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    pub unlocked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unlocked_at: Option<String>,
}

#[derive(Serialize)]
struct AchievementsListResponse {
    achievements: Vec<AchievementRow>,
}

#[derive(Serialize)]
struct UnlockResponse {
    success: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    newly_unlocked: bool,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/achievements", get(list_my_achievements))
        .route("/achievements/{code}/unlock", post(unlock_achievement))
        .route("/users/{user_id}/achievements", get(list_user_achievements))
}

async fn list_my_achievements(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<AchievementsListResponse>, ApiError> {
    let user_id = extract_user_id(&state, &headers)?;
    let achievements = fetch_achievements_for_user(&state.pool, user_id).await?;
    Ok(Json(AchievementsListResponse { achievements }))
}

async fn list_user_achievements(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(target_user_id): Path<Uuid>,
) -> Result<Json<AchievementsListResponse>, ApiError> {
    let viewer_id = extract_user_id(&state, &headers)?;
    ensure_can_view_user(&state.pool, viewer_id, target_user_id).await?;
    let achievements = fetch_achievements_for_user(&state.pool, target_user_id).await?;
    Ok(Json(AchievementsListResponse { achievements }))
}

async fn unlock_achievement(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(code): Path<String>,
) -> Result<Json<UnlockResponse>, ApiError> {
    let user_id = extract_user_id(&state, &headers)?;
    let code = code.trim();
    if code.is_empty() {
        return Err(ApiError::bad_request("achievement code is empty"));
    }

    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM achievements WHERE code = $1",
    )
    .bind(code)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if exists == 0 {
        return Err(ApiError::NotFound);
    }

    let newly_unlocked = try_unlock(&state.pool, user_id, code).await?;
    Ok(Json(UnlockResponse {
        success: true,
        newly_unlocked,
    }))
}

async fn fetch_achievements_for_user(
    pool: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<Vec<AchievementRow>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT a.code,
               a.title,
               a.description,
               a.icon_url,
               ua.unlocked_at
        FROM achievements a
        LEFT JOIN user_achievements ua
            ON ua.achievement_id = a.id AND ua.user_id = $1
        ORDER BY a.created_at ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let unlocked_at: Option<chrono::DateTime<chrono::Utc>> = row.get("unlocked_at");
            AchievementRow {
                code: row.get("code"),
                title: row.get("title"),
                description: row.get("description"),
                icon_url: row.get("icon_url"),
                unlocked: unlocked_at.is_some(),
                unlocked_at: unlocked_at.map(|dt| dt.to_rfc3339()),
            }
        })
        .collect())
}
