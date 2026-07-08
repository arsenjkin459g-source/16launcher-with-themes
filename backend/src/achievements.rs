use std::path::{Path, PathBuf};

use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Debug, Deserialize)]
struct AchievementsFile {
    #[serde(default = "default_version")]
    version: u32,
    achievements: Vec<AchievementDefinition>,
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
struct AchievementDefinition {
    code: String,
    title: String,
    description: String,
    #[serde(default)]
    icon_url: Option<String>,
}

pub fn catalog_path() -> PathBuf {
    std::env::var("ACHIEVEMENTS_JSON_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/achievements.json"))
}

fn load_catalog(path: &Path) -> Result<Vec<AchievementDefinition>, ApiError> {
    let raw = std::fs::read_to_string(path).map_err(|e| {
        ApiError::Internal(format!("failed to read achievements catalog {}: {e}", path.display()))
    })?;

    let file: AchievementsFile = serde_json::from_str(&raw).map_err(|e| {
        ApiError::Internal(format!("invalid achievements catalog {}: {e}", path.display()))
    })?;

    if file.version != 1 {
        return Err(ApiError::Internal(format!(
            "unsupported achievements catalog version: {}",
            file.version
        )));
    }

    let mut seen = std::collections::HashSet::new();
    for item in &file.achievements {
        let code = item.code.trim();
        if code.is_empty() {
            return Err(ApiError::Internal(
                "achievement entry has empty code".into(),
            ));
        }
        if item.title.trim().is_empty() {
            return Err(ApiError::Internal(format!(
                "achievement '{code}' has empty title"
            )));
        }
        if item.description.trim().is_empty() {
            return Err(ApiError::Internal(format!(
                "achievement '{code}' has empty description"
            )));
        }
        if !seen.insert(code.to_string()) {
            return Err(ApiError::Internal(format!(
                "duplicate achievement code: {code}"
            )));
        }
    }

    Ok(file.achievements)
}

/// Upserts achievement definitions from JSON into PostgreSQL.
pub async fn sync_catalog(pool: &PgPool) -> Result<(), ApiError> {
    let path = catalog_path();
    let definitions = load_catalog(&path)?;

    for item in definitions {
        let code = item.code.trim();
        let icon_url = item
            .icon_url
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());

        sqlx::query(
            r#"
            INSERT INTO achievements (code, title, description, icon_url)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (code) DO UPDATE
            SET title = EXCLUDED.title,
                description = EXCLUDED.description,
                icon_url = EXCLUDED.icon_url
            "#,
        )
        .bind(code)
        .bind(item.title.trim())
        .bind(item.description.trim())
        .bind(icon_url)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    }

    tracing::info!(
        "achievements catalog synced from {}",
        path.display()
    );
    Ok(())
}

/// Unlocks an achievement for a user. Returns `true` if newly unlocked.
pub async fn try_unlock(pool: &PgPool, user_id: Uuid, code: &str) -> Result<bool, ApiError> {
    let result = sqlx::query(
        r#"
        INSERT INTO user_achievements (user_id, achievement_id)
        SELECT $1, a.id
        FROM achievements a
        WHERE a.code = $2
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(code)
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(result.rows_affected() > 0)
}

pub async fn check_friend_achievements(pool: &PgPool, user_id: Uuid) -> Result<(), ApiError> {
    try_unlock(pool, user_id, "first_friend").await?;

    let friend_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM friends WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if friend_count >= 5 {
        try_unlock(pool, user_id, "five_friends").await?;
    }

    Ok(())
}
