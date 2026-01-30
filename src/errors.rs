use std::fmt;

#[derive(Debug)]
pub enum AppError {
    NotInitialized,
    GhNotAuthenticated,
    DatabaseError(String),
    IoError(std::io::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotInitialized => write!(f, "Run 'claude-manager --init' first"),
            AppError::GhNotAuthenticated => write!(f, "Run 'gh auth login' first"),
            AppError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            AppError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::IoError(e)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::DatabaseError(e.to_string())
    }
}
