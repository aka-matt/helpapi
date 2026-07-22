//! TLS support: loads certificates and private keys from JKS (Java KeyStore)
//! files and builds a rustls server configuration for HTTPS serving.

use axum_server::tls_rustls::RustlsConfig;

use crate::error::HttpError;
use crate::server::TlsSettings;

/// Loads the JKS keystore described by `settings` and builds a rustls
/// configuration from the first private key entry it contains.
///
/// - `keystore_password` unlocks the keystore itself (integrity check).
/// - `key_password` decrypts the private key entry; when omitted, the
///   keystore password is used (matching `keytool` defaults).
///
/// # Errors
///
/// Returns [`HttpError::TlsError`] if the file cannot be read, the keystore
/// cannot be parsed or unlocked, no private key entry exists, the key cannot
/// be decrypted, or the certificate/key material is invalid.
pub async fn load_rustls_config(settings: &TlsSettings) -> Result<RustlsConfig, HttpError> {
    let data = std::fs::read(&settings.keystore_file).map_err(|e| HttpError::TlsError {
        reason: format!(
            "failed to read keystore file '{}': {}",
            settings.keystore_file, e
        ),
    })?;

    let mut keystore = jks::KeyStore::new();
    keystore
        .load(
            std::io::Cursor::new(data),
            settings.keystore_password.as_bytes(),
        )
        .map_err(|e| HttpError::TlsError {
            reason: format!(
                "failed to load keystore '{}' (check keystore_password): {}",
                settings.keystore_file, e
            ),
        })?;

    // Pick the first private key entry in the keystore.
    let alias = keystore
        .aliases()
        .into_iter()
        .find(|a| keystore.is_private_key_entry(a))
        .ok_or_else(|| HttpError::TlsError {
            reason: format!(
                "keystore '{}' contains no private key entry",
                settings.keystore_file
            ),
        })?;

    let key_password = settings
        .key_password
        .as_deref()
        .unwrap_or(&settings.keystore_password);

    let entry = keystore
        .get_private_key_entry(&alias, key_password.as_bytes())
        .map_err(|e| HttpError::TlsError {
            reason: format!(
                "failed to decrypt private key entry '{}' (check key_password): {}",
                alias, e
            ),
        })?;

    let certs: Vec<Vec<u8>> = entry
        .certificate_chain
        .iter()
        .map(|c| c.content.clone())
        .collect();

    if certs.is_empty() {
        return Err(HttpError::TlsError {
            reason: format!("private key entry '{}' has no certificate chain", alias),
        });
    }

    RustlsConfig::from_der(certs, entry.private_key)
        .await
        .map_err(|e| HttpError::TlsError {
            reason: format!("invalid certificate or private key: {}", e),
        })
}
