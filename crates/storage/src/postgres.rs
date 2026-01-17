use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, Row, postgres::PgPoolOptions};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PgError {
    #[error("Database error: {0}")]
    DbError(#[from] sqlx::Error),
}

/// User Group definition (e.g., "Premium Asia", "Free US")
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExitGroup {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
}

/// User definition
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub token_hash: String,
    pub group_id: i32,
    pub balance: i64, // simplified billing
}

/// Postgres Client helper
#[derive(Clone)]
pub struct PgClient {
    pool: Pool<Postgres>,
}

impl PgClient {
    pub async fn new(url: &str) -> Result<Self, PgError> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .acquire_timeout(Duration::from_secs(3))
            .connect(url)
            .await?;

        Ok(Self { pool })
    }

    /// Initialize schema
    pub async fn migrate(&self) -> Result<(), PgError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS exit_groups (
                id SERIAL PRIMARY KEY,
                name VARCHAR(50) NOT NULL UNIQUE,
                description TEXT
            );

            CREATE TABLE IF NOT EXISTS users (
                id BIGSERIAL PRIMARY KEY,
                username VARCHAR(100) NOT NULL UNIQUE,
                token_hash VARCHAR(255) NOT NULL,
                group_id INT REFERENCES exit_groups(id),
                balance BIGINT DEFAULT 0,
                created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS billing_logs (
                id BIGSERIAL PRIMARY KEY,
                user_id BIGINT REFERENCES users(id),
                bytes_used BIGINT NOT NULL,
                timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW()
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Seed default group if empty
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM exit_groups")
            .fetch_one(&self.pool)
            .await?;

        if count == 0 {
            sqlx::query(
                "INSERT INTO exit_groups (name, description) VALUES ('default', 'Default Group')",
            )
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub async fn get_user_by_token(&self, token: &str) -> Result<Option<User>, PgError> {
        // Note: In production, use bcrypt/argon2 to verify token_hash
        // Current implementation does direct hash comparison for simplicity
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE token_hash = $1")
            .bind(token)
            .fetch_optional(&self.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn record_usage(&self, user_id: i64, bytes: u64) -> Result<(), PgError> {
        sqlx::query("INSERT INTO billing_logs (user_id, bytes_used) VALUES ($1, $2)")
            .bind(user_id)
            .bind(bytes as i64)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
