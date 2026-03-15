use thiserror::Error;

/// Errors that can occur during diff parsing and processing.
#[derive(Debug, Error)]
pub enum DiffError {
    /// The diff text could not be parsed at the given line.
    #[error("failed to parse diff at line {line}: {reason}")]
    ParseError {
        /// Line number where parsing failed.
        line: usize,
        /// Human-readable reason for the failure.
        reason: String,
    },

    /// A file referenced in the diff does not exist in the repository.
    #[error("file not found in repository: {path}")]
    FileNotFound {
        /// The missing file path.
        path: String,
    },

    /// An underlying `git2` library error.
    #[error(transparent)]
    Git(#[from] git2::Error),
}

/// Errors related to Git repository operations.
#[derive(Debug, Error)]
pub enum GitError {
    /// The current directory is not inside a Git repository.
    #[error("repository not found")]
    RepoNotFound,

    /// The requested branch does not exist.
    #[error("branch not found: {name}")]
    BranchNotFound {
        /// Name of the missing branch.
        name: String,
    },

    /// Unable to automatically detect the base branch for comparison.
    #[error("failed to detect base branch")]
    NoBaseBranch,

    /// An underlying `git2` library error.
    #[error(transparent)]
    Git2(#[from] git2::Error),
}

/// Errors related to reading / writing persistent state files.
#[derive(Debug, Error)]
pub enum StateError {
    /// Could not read the state file from disk.
    #[error("failed to read state file: {path}")]
    Read {
        /// Path of the state file.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Could not write the state file to disk.
    #[error("failed to write state file: {path}")]
    Write {
        /// Path of the state file.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Could not deserialise the state file (corrupted or incompatible format).
    #[error("failed to parse state file: {path}")]
    Parse {
        /// Path of the state file.
        path: String,
        /// Underlying JSON parse error.
        #[source]
        source: serde_json::Error,
    },
}

/// Errors that can occur during source-file parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    /// The file's language is not supported by the parser.
    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),

    /// The parser encountered a syntax error.
    #[error("failed to parse file: {path}: {reason}")]
    SyntaxError {
        /// Path of the file that failed to parse.
        path: String,
        /// Human-readable description of the syntax error.
        reason: String,
    },
}
