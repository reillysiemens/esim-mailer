use crate::Args;
use lettre::message::header;
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::{Message, SmtpTransport, Transport};
use std::error::Error;
use std::fmt::Display;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use uuid;

/// Errors that can occur during email operations.
#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    /// An unsupported email provider was specified
    #[error("Unsupported email provider: {0}")]
    UnsupportedProvider(#[from] ParseProviderError),
    /// Failed to parse or build email content
    #[error("Email message error: {0}")]
    MessageError(String),
    /// Network/SMTP connection failed
    #[error("SMTP error: {0}")]
    SmtpError(String),
    /// File system operations failed
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// An error which can be returned when parsing a provider from an email address.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("No supported email provider for '{0}'")]
pub struct ParseProviderError(String);

/// An email provider.
#[derive(Debug, PartialEq, Eq)]
pub enum Provider {
    Gmail,
    Outlook,
}

impl FromStr for Provider {
    type Err = ParseProviderError;

    fn from_str(email: &str) -> Result<Self, Self::Err> {
        match email.rsplit_once('@') {
            Some((_, "gmail.com")) => Ok(Self::Gmail),
            Some((_, "outlook.com" | "hotmail.com")) => Ok(Self::Outlook),
            _ => Err(ParseProviderError(email.to_string())),
        }
    }
}

impl Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gmail => write!(f, "Gmail"),
            Self::Outlook => write!(f, "Outlook"),
        }
    }
}

pub struct EmailTemplate {
    subject_template: &'static str,
    body_template: &'static str,
}

impl Default for EmailTemplate {
    fn default() -> Self {
        Self::new()
    }
}

impl EmailTemplate {
    pub fn new() -> Self {
        Self {
            subject_template: "[{{provider}}] {{location}} eSIM",
            body_template: include_str!("../templates/email_template.html"),
        }
    }

    pub fn subject(&self, args: &Args, count: usize) -> String {
        let subject = self
            .subject_template
            .replace("{{provider}}", &args.provider)
            .replace("{{location}}", &args.location);
        format!("{} - {}", subject, count)
    }

    pub fn body(&self, args: &Args) -> String {
        self.body_template
            .replace("{{provider}}", &args.provider)
            .replace("{{name}}", &args.name)
            .replace("{{data_amount}}", &args.data_amount)
            .replace("{{time_period}}", &args.time_period)
            .replace("{{location}}", &args.location)
    }
}

pub fn send_email(
    args: &Args,
    token: String,
    image_path: &Path,
    count: usize,
) -> Result<(), EmailError> {
    let email_from = &args.email_from;
    let email_to = &args.email_to;

    // Get template content
    let template = EmailTemplate::new();

    // Read image file
    let image_data = fs::read(image_path)?;

    // Get subject and body content
    let subject = template.subject(args, count);
    // Generate a unique Content-ID for the image
    let content_id = format!("qr_image_cid@{}", uuid::Uuid::new_v4());

    // Get the body content and replace the QR_CID placeholder with the actual Content-ID
    let body_content = template.body(args);
    let body = body_content.replace("{{QR_CID}}", &content_id);

    // Create multipart email with HTML body and image attachment
    let mut email_builder =
        Message::builder()
            .from(email_from.parse().map_err(|e| {
                EmailError::MessageError(format!("Invalid from email address: {}", e))
            })?)
            .to(email_to.parse().map_err(|e| {
                EmailError::MessageError(format!("Invalid to email address: {}", e))
            })?)
            .subject(subject);

    // Add BCC if provided and not empty
    if let Some(bcc) = &args.bcc
        && !bcc.is_empty()
    {
        email_builder =
            email_builder.bcc(bcc.parse().map_err(|e| {
                EmailError::MessageError(format!("Invalid BCC email address: {}", e))
            })?);
    }

    // Build the email with multipart/related content
    let email = email_builder
        .multipart(
            lettre::message::MultiPart::related()
                .singlepart(
                    lettre::message::SinglePart::builder()
                        .header(header::ContentType::TEXT_HTML)
                        .body(body),
                )
                .singlepart(lettre::message::Attachment::new_inline(content_id).body(
                    image_data,
                    header::ContentType::parse("image/png").map_err(|e| {
                        EmailError::MessageError(format!("Invalid content type: {}", e))
                    })?,
                )),
        )
        .map_err(|e| EmailError::MessageError(format!("Failed to build email: {}", e)))?;

    // Configure SMTP client with TLS
    let provider: Provider = email_from.parse()?;
    let mailer = configure_mailer(&provider, email_from, token)?;

    // Send the email
    match mailer.send(&email) {
        Ok(_) => {
            println!("Email sent successfully!");
            Ok(())
        }
        Err(e) => {
            eprintln!("Could not send email: {:?}", e);
            if let Some(source) = e.source() {
                eprintln!("Error source: {:?}", source);
            }
            Err(EmailError::SmtpError(format!(
                "Could not send email: {}",
                e
            )))
        }
    }
}

fn configure_mailer(
    provider: &Provider,
    email_address: &str,
    token: String,
) -> Result<SmtpTransport, EmailError> {
    match provider {
        Provider::Gmail => Ok(SmtpTransport::relay("smtp.gmail.com")
            .map_err(|e| EmailError::SmtpError(format!("Failed to connect to Gmail SMTP: {}", e)))?
            .credentials(Credentials::new(email_address.to_string(), token))
            .authentication(vec![Mechanism::Xoauth2])
            .port(587)
            .tls(lettre::transport::smtp::client::Tls::Required(
                lettre::transport::smtp::client::TlsParameters::new("smtp.gmail.com".to_string())
                    .map_err(|e| {
                    EmailError::SmtpError(format!("Failed to configure TLS for Gmail: {}", e))
                })?,
            ))
            .build()),
        Provider::Outlook => Ok(SmtpTransport::relay("smtp-mail.outlook.com")
            .map_err(|e| {
                EmailError::SmtpError(format!("Failed to connect to Outlook SMTP: {}", e))
            })?
            .credentials(Credentials::new(email_address.to_string(), token))
            .authentication(vec![Mechanism::Xoauth2])
            .port(587)
            .tls(lettre::transport::smtp::client::Tls::Required(
                lettre::transport::smtp::client::TlsParameters::new(
                    "smtp-mail.outlook.com".to_string(),
                )
                .map_err(|e| {
                    EmailError::SmtpError(format!("Failed to configure TLS for Outlook: {}", e))
                })?,
            ))
            .build()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_template_subject() {
        let template = EmailTemplate::new();
        let args = Args {
            email_from: "sender@example.com".to_string(),
            email_to: "recipient@example.com".to_string(),
            bcc: None,
            provider: "TestProvider".to_string(),
            name: "John".to_string(),
            data_amount: "5GB".to_string(),
            time_period: "30 days".to_string(),
            location: "Egypt".to_string(),
        };
        let result = template.subject(&args, 1);
        assert_eq!(result, "[TestProvider] Egypt eSIM - 1");
    }

    #[test]
    fn test_email_template_body() {
        let template = EmailTemplate::new();
        let args = Args {
            email_from: "sender@example.com".to_string(),
            email_to: "recipient@example.com".to_string(),
            bcc: None,
            provider: "TestProvider".to_string(),
            name: "John".to_string(),
            data_amount: "5GB".to_string(),
            time_period: "30 days".to_string(),
            location: "Egypt".to_string(),
        };
        let result = template.body(&args);
        assert!(result.contains("John"));
        assert!(result.contains("TestProvider"));
        assert!(result.contains("5GB"));
        assert!(result.contains("30 days"));
        assert!(result.contains("Egypt"));
    }

    #[test]
    fn parse_valid_provider() {
        let gmail = "foobar@gmail.com".parse::<Provider>();
        assert_eq!(gmail, Ok(Provider::Gmail));

        let outlook = "foobar@outlook.com".parse::<Provider>();
        assert_eq!(outlook, Ok(Provider::Outlook));

        let hotmail = "foobar@hotmail.com".parse::<Provider>();
        assert_eq!(hotmail, Ok(Provider::Outlook));
    }

    #[test]
    fn parse_invalid_provider() {
        let result = "foobar@yahoo.com".parse::<Provider>();
        assert_eq!(result, Err(ParseProviderError("foobar@yahoo.com".into())));
    }

    #[test]
    fn test_configure_mailer_gmail() {
        let result = configure_mailer(&Provider::Gmail, "test@gmail.com", "token".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_configure_mailer_outlook() {
        let result = configure_mailer(&Provider::Outlook, "test@outlook.com", "token".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_provider_display() {
        assert_eq!(Provider::Gmail.to_string(), "Gmail");
        assert_eq!(Provider::Outlook.to_string(), "Outlook");
    }

    #[test]
    fn test_send_email() -> Result<(), EmailError> {
        // Create a temporary test image
        let temp_dir = std::env::temp_dir();
        let image_path = temp_dir.join("test_image.png");
        fs::write(&image_path, b"fake image data")?;

        let args = Args {
            email_from: "test@gmail.com".to_string(),
            email_to: "recipient@example.com".to_string(),
            bcc: Some("bcc@example.com".to_string()),
            provider: "TestProvider".to_string(),
            name: "Test User".to_string(),
            data_amount: "1GB".to_string(),
            time_period: "7 days".to_string(),
            location: "TestLocation".to_string(),
        };

        // Test the function - it should fail when trying to send
        let result = send_email(&args, "fake_token".to_string(), &image_path, 1);

        // Clean up the temporary file
        fs::remove_file(image_path)?;

        // We expect an error from the SMTP client
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Could not send email"));
        assert!(
            err.to_string()
                .contains("mechanism does not expect a challenge")
        );

        Ok(())
    }

    #[test]
    fn test_send_email_invalid_provider() {
        let args = Args {
            email_from: "test@unsupported.com".to_string(),
            email_to: "recipient@example.com".to_string(),
            bcc: None,
            provider: "TestProvider".to_string(),
            name: "Test User".to_string(),
            data_amount: "1GB".to_string(),
            time_period: "7 days".to_string(),
            location: "TestLocation".to_string(),
        };

        // Create a temporary test image first
        let temp_dir = std::env::temp_dir();
        let image_path = temp_dir.join("test_image2.png");
        fs::write(&image_path, b"fake image data").expect("Failed to write test file");

        let result = send_email(&args, "fake_token".to_string(), &image_path, 1);

        // Clean up
        fs::remove_file(image_path).expect("Failed to clean up test file");

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported email provider")
        );
    }
}
