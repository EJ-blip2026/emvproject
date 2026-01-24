pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn run() {
    println!("emvproject library loaded (v{})", version());
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
