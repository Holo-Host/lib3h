//! Lib3h Crypto API CryptoError module

/// Represents an error generated by the cryptography system
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CryptoError {
    Generic(String),
    OutputLength(String),
    OutOfMemory,
    WriteOverflow,
    BadHashSize,
    BadSaltSize,
    BadSeedSize,
    BadPublicKeySize,
    BadSecretKeySize,
    BadSignatureSize,
}

impl CryptoError {
    pub fn new(msg: &str) -> Self {
        CryptoError::Generic(msg.to_string())
    }
}

impl std::error::Error for CryptoError {
    fn description(&self) -> &str {
        "CryptoError"
    }
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// represents a Result object returned by an api in the cryptography system
pub type CryptoResult<T> = Result<T, CryptoError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_should_display_types() {
        assert_eq!(
            "Generic(\"bla\")",
            &format!("{}", CryptoError::Generic("bla".to_string()))
        );
        assert_eq!(
            "OutputLength(\"bla\")",
            &format!("{}", CryptoError::OutputLength("bla".to_string()))
        );
        assert_eq!("OutOfMemory", &format!("{}", CryptoError::OutOfMemory));
    }
}
