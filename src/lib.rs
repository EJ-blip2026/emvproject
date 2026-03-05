pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn run() {
    println!("emvproject library loaded (v{})", version());
}

pub fn redact_sensitive(input: &str) -> String {
    if input.len() <= 8 {
        return "****".to_string();
    }

    format!("{}...{}", &input[..4], &input[input.len() - 4..])
}

pub fn redact(token: &str) -> String {
    if token.len() < 10 {
        return "****".to_string();
    }

    format!("{}...{}", &token[..4], &token[token.len() - 4..])
}

pub fn redact_secret(secret: &str) -> String {
    if secret.len() <= 8 {
        return "****".to_string();
    }

    format!("{}...{}", &secret[..4], &secret[secret.len() - 4..])
}

/// Example error for vault parsing.
#[derive(Debug)]
pub enum VaultError {
    InvalidOrUnsupportedFormat,
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::InvalidOrUnsupportedFormat => {
                write!(f, "Invalid or unsupported vault format")
            }
        }
    }
}

impl std::error::Error for VaultError {}
