/// Error types for the eSIM Mailer application.
///
/// This module provides a comprehensive error handling system that replaces
/// the previous pattern of mapping all errors to `std::io::Error`.
use std::fmt;

/// The main error type for eSIM Mailer operations.
///
/// This enum captures all the different ways operations can fail in the application,
/// providing better error context than generic `io::Error` mapping.
#[derive(Debug)]
pub enum EsimMailerError {
    /// An unsupported email provider was specified
    UnsupportedProvider(String),

    /// OAuth authentication failed
    OAuthError(String),

    /// Email sending operation failed
    EmailError(String),

    /// Template processing failed
    TemplateError(String),

    /// File system operations failed
    IoError(std::io::Error),

    /// Network operations failed
    NetworkError(String),

    /// Encryption/decryption operations failed
    CryptoError(String),

    /// Configuration is invalid or missing
    ConfigError(String),

    /// GUI operations failed
    GuiError(String),
}

impl fmt::Display for EsimMailerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EsimMailerError::UnsupportedProvider(provider) => {
                write!(f, "Unsupported email provider: {}", provider)
            }
            EsimMailerError::OAuthError(msg) => {
                write!(f, "OAuth authentication failed: {}", msg)
            }
            EsimMailerError::EmailError(msg) => {
                write!(f, "Email operation failed: {}", msg)
            }
            EsimMailerError::TemplateError(msg) => {
                write!(f, "Template processing failed: {}", msg)
            }
            EsimMailerError::IoError(err) => {
                write!(f, "IO operation failed: {}", err)
            }
            EsimMailerError::NetworkError(msg) => {
                write!(f, "Network operation failed: {}", msg)
            }
            EsimMailerError::CryptoError(msg) => {
                write!(f, "Cryptographic operation failed: {}", msg)
            }
            EsimMailerError::ConfigError(msg) => {
                write!(f, "Configuration error: {}", msg)
            }
            EsimMailerError::GuiError(msg) => {
                write!(f, "GUI error: {}", msg)
            }
        }
    }
}

impl std::error::Error for EsimMailerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EsimMailerError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

// Conversion from std::io::Error
impl From<std::io::Error> for EsimMailerError {
    fn from(err: std::io::Error) -> Self {
        EsimMailerError::IoError(err)
    }
}

/// A type alias for `Result<T, EsimMailerError>`.
///
/// This saves repetition and makes function signatures more readable.
pub type Result<T> = std::result::Result<T, EsimMailerError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_error_display() {
        let err = EsimMailerError::UnsupportedProvider("unknown".to_string());
        assert_eq!(format!("{}", err), "Unsupported email provider: unknown");
    }

    #[test]
    fn test_error_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let esim_err = EsimMailerError::IoError(io_err);
        assert!(esim_err.source().is_some());
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let esim_err: EsimMailerError = io_err.into();

        match esim_err {
            EsimMailerError::IoError(_) => (),
            _ => panic!("Expected IoError variant"),
        }
    }

    #[test]
    fn test_result_alias() {
        fn example_function() -> Result<String> {
            Ok("success".to_string())
        }

        assert!(example_function().is_ok());
    }
}
