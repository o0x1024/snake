use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce, aead::{Aead, KeyInit}};
use async_trait::async_trait;
use rand::RngCore;

use crate::error::{AuroraResult, CryptoError};
use super::traits::Encryptor;

pub struct ChaCha20Poly1305Encryptor;

impl ChaCha20Poly1305Encryptor {
    pub fn new() -> Self {
        Self
    }

    fn generate_nonce() -> [u8; 12] {
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        nonce
    }
}

#[async_trait]
impl Encryptor for ChaCha20Poly1305Encryptor {
    async fn encrypt(&self, data: &[u8], key: &[u8]) -> AuroraResult<Vec<u8>> {
        if key.len() != 32 {
            return Err(CryptoError::InvalidKey.into());
        }

        let key = Key::from_slice(key);
        let cipher = ChaCha20Poly1305::new(key);
        
        let nonce_bytes = Self::generate_nonce();
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|_| CryptoError::Encryption)?;
        
        // Prepend nonce to ciphertext
        let mut result = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }

    async fn decrypt(&self, encrypted_data: &[u8], key: &[u8]) -> AuroraResult<Vec<u8>> {
        if key.len() != 32 {
            return Err(CryptoError::InvalidKey.into());
        }

        if encrypted_data.len() < 12 {
            return Err(CryptoError::Decryption.into());
        }

        let key = Key::from_slice(key);
        let cipher = ChaCha20Poly1305::new(key);
        
        let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        
        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|_| CryptoError::Decryption)?;
        
        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_chacha20_poly1305_encryption_decryption() {
        let encryptor = ChaCha20Poly1305Encryptor::new();
        let key = [0u8; 32]; // Test key
        let data = b"Hello, ChaCha20!";

        let encrypted = encryptor.encrypt(data, &key).await.unwrap();
        let decrypted = encryptor.decrypt(&encrypted, &key).await.unwrap();

        assert_eq!(data, decrypted.as_slice());
    }
}