use std::fmt;

/// Core error type for the aft binary.
///
/// Each variant maps to a structured error response with a `code` string
/// and a human-readable `message`.
#[derive(Debug)]
pub enum AftError {
    SymbolNotFound {
        name: String,
        file: String,
    },
    AmbiguousSymbol {
        name: String,
        candidates: Vec<String>,
    },
    ParseError {
        message: String,
    },
    FileNotFound {
        path: String,
    },
    InvalidRequest {
        message: String,
    },
    CheckpointNotFound {
        name: String,
    },
    NoUndoHistory {
        path: String,
    },
}

impl AftError {
    /// Returns the error code string used in JSON error responses.
    pub fn code(&self) -> &'static str {
        match self {
            AftError::SymbolNotFound { .. } => "symbol_not_found",
            AftError::AmbiguousSymbol { .. } => "ambiguous_symbol",
            AftError::ParseError { .. } => "parse_error",
            AftError::FileNotFound { .. } => "file_not_found",
            AftError::InvalidRequest { .. } => "invalid_request",
            AftError::CheckpointNotFound { .. } => "checkpoint_not_found",
            AftError::NoUndoHistory { .. } => "no_undo_history",
        }
    }

    /// Produces a `serde_json::Value` suitable for the error portion of a response.
    ///
    /// Shape: `{ "code": "...", "message": "..." }`
    pub fn to_error_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": self.code(),
            "message": self.to_string(),
        })
    }
}

impl fmt::Display for AftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AftError::SymbolNotFound { name, file } => {
                write!(f, "symbol '{}' not found in {}", name, file)
            }
            AftError::AmbiguousSymbol { name, candidates } => {
                write!(
                    f,
                    "symbol '{}' is ambiguous, candidates: [{}]",
                    name,
                    candidates.join(", ")
                )
            }
            AftError::ParseError { message } => {
                write!(f, "parse error: {}", message)
            }
            AftError::FileNotFound { path } => {
                write!(f, "file not found: {}", path)
            }
            AftError::InvalidRequest { message } => {
                write!(f, "invalid request: {}", message)
            }
            AftError::CheckpointNotFound { name } => {
                write!(f, "checkpoint not found: {}", name)
            }
            AftError::NoUndoHistory { path } => {
                write!(f, "no undo history for: {}", path)
            }
        }
    }
}

impl std::error::Error for AftError {}
