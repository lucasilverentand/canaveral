//! Age encryption for the team vault

use age::secrecy::ExposeSecret;
use std::io::{Read, Write};
use thiserror::Error;

/// Encryption-related errors
#[derive(Debug, Error)]
pub enum EncryptionError {
    /// Failed to parse public key
    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),

    /// Failed to parse private key
    #[error("Invalid private key: {0}")]
    InvalidPrivateKey(String),

    /// Encryption failed
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    /// Decryption failed
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    /// No recipients specified
    #[error("No recipients specified for encryption")]
    NoRecipients,

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// An Age keypair for encryption/decryption
#[derive(Debug, Clone)]
pub struct KeyPair {
    /// The public key (safe to share)
    pub public_key: String,
    /// The private key (keep secret!)
    pub private_key: String,
}

impl KeyPair {
    /// Get the public key in a format suitable for display
    pub fn public_key_display(&self) -> &str {
        &self.public_key
    }
}

/// Generate a new Age keypair
pub fn generate_keypair() -> KeyPair {
    let identity = age::x25519::Identity::generate();
    let public_key = identity.to_public().to_string();
    let private_key = identity.to_string().expose_secret().to_string();

    KeyPair {
        public_key,
        private_key,
    }
}

/// Encrypt data for multiple recipients
///
/// # Arguments
/// * `data` - The data to encrypt
/// * `recipients` - Age public keys of recipients who can decrypt
///
/// # Returns
/// Armored (ASCII) encrypted data
pub fn encrypt_data(data: &[u8], recipients: &[String]) -> Result<String, EncryptionError> {
    if recipients.is_empty() {
        return Err(EncryptionError::NoRecipients);
    }

    // Parse all recipient public keys
    let parsed_recipients: Vec<Box<dyn age::Recipient + Send>> = recipients
        .iter()
        .map(|r| {
            r.parse::<age::x25519::Recipient>()
                .map(|r| Box::new(r) as Box<dyn age::Recipient + Send>)
                .map_err(|e| EncryptionError::InvalidPublicKey(format!("{}: {}", r, e)))
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Create encryptor
    let encryptor = age::Encryptor::with_recipients(parsed_recipients)
        .ok_or_else(|| EncryptionError::EncryptionFailed("Failed to create encryptor".to_string()))?;

    // Encrypt to armored output
    let mut output = Vec::new();
    let armor_writer = age::armor::ArmoredWriter::wrap_output(&mut output, age::armor::Format::AsciiArmor)
        .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

    let mut writer = encryptor
        .wrap_output(armor_writer)
        .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

    writer.write_all(data)?;
    let armor_writer = writer
        .finish()
        .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

    armor_writer
        .finish()
        .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

    String::from_utf8(output)
        .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))
}

/// Decrypt data using a private key
///
/// # Arguments
/// * `encrypted` - The armored encrypted data
/// * `private_key` - The Age private key
///
/// # Returns
/// Decrypted data
pub fn decrypt_data(encrypted: &str, private_key: &str) -> Result<Vec<u8>, EncryptionError> {
    // Parse the private key
    let identity: age::x25519::Identity = private_key
        .parse()
        .map_err(|e| EncryptionError::InvalidPrivateKey(format!("{}", e)))?;

    // Create decryptor from armored input
    let armor_reader = age::armor::ArmoredReader::new(encrypted.as_bytes());

    let decryptor = match age::Decryptor::new(armor_reader) {
        Ok(age::Decryptor::Recipients(d)) => d,
        Ok(_) => {
            return Err(EncryptionError::DecryptionFailed(
                "Unexpected passphrase-encrypted data".to_string(),
            ))
        }
        Err(e) => return Err(EncryptionError::DecryptionFailed(e.to_string())),
    };

    // Decrypt
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;

    let mut output = Vec::new();
    reader.read_to_end(&mut output)?;

    Ok(output)
}

/// Re-encrypt data for a new set of recipients
///
/// Useful when adding/removing team members
pub fn reencrypt_data(
    encrypted: &str,
    private_key: &str,
    new_recipients: &[String],
) -> Result<String, EncryptionError> {
    let decrypted = decrypt_data(encrypted, private_key)?;
    encrypt_data(&decrypted, new_recipients)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let keypair = generate_keypair();

        assert!(keypair.public_key.starts_with("age1"));
        assert!(keypair.private_key.starts_with("AGE-SECRET-KEY-"));
    }

    #[test]
    fn test_encrypt_decrypt() {
        let keypair = generate_keypair();
        let data = b"Hello, World!";

        let encrypted = encrypt_data(data, &[keypair.public_key.clone()]).unwrap();
        assert!(encrypted.contains("-----BEGIN AGE ENCRYPTED FILE-----"));

        let decrypted = decrypt_data(&encrypted, &keypair.private_key).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_multi_recipient() {
        let keypair1 = generate_keypair();
        let keypair2 = generate_keypair();
        let data = b"Secret message";

        let encrypted = encrypt_data(
            data,
            &[keypair1.public_key.clone(), keypair2.public_key.clone()],
        )
        .unwrap();

        // Both recipients should be able to decrypt
        let decrypted1 = decrypt_data(&encrypted, &keypair1.private_key).unwrap();
        let decrypted2 = decrypt_data(&encrypted, &keypair2.private_key).unwrap();

        assert_eq!(decrypted1, data);
        assert_eq!(decrypted2, data);
    }

    #[test]
    fn test_wrong_key_fails() {
        let keypair1 = generate_keypair();
        let keypair2 = generate_keypair();
        let data = b"Secret";

        let encrypted = encrypt_data(data, &[keypair1.public_key.clone()]).unwrap();

        // Wrong key should fail
        let result = decrypt_data(&encrypted, &keypair2.private_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_reencrypt() {
        let keypair1 = generate_keypair();
        let keypair2 = generate_keypair();
        let data = b"Reencrypt me";

        // Encrypt for keypair1
        let encrypted = encrypt_data(data, &[keypair1.public_key.clone()]).unwrap();

        // Reencrypt for keypair2
        let reencrypted = reencrypt_data(
            &encrypted,
            &keypair1.private_key,
            &[keypair2.public_key.clone()],
        )
        .unwrap();

        // keypair1 should no longer be able to decrypt
        assert!(decrypt_data(&reencrypted, &keypair1.private_key).is_err());

        // keypair2 should be able to decrypt
        let decrypted = decrypt_data(&reencrypted, &keypair2.private_key).unwrap();
        assert_eq!(decrypted, data);
    }
}
