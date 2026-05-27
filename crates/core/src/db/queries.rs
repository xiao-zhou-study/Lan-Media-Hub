use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("SQL error: {0}")]
    SqlError(#[from] sqlx::Error),

    #[error("Connection pool error: {0}")]
    PoolError(String),

    #[error("Migration error: {0}")]
    MigrationError(String),

    #[error("Record not found")]
    NotFound,

    #[error("Invalid data: {0}")]
    InvalidData(String),
}