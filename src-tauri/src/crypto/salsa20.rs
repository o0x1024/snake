use salsa20::{Salsa20, cipher::{KeyIvInit, StreamCipher}};
use async_trait::async_trait;
use rand::RngCore;
use ring::digest;

use crate::error::{AuroraResult, CryptoError};
use super::traits::Encryptor;

pub struct Salsa20Encryptor;

impl Salsa20Encryptor {
    pub fn new() -> Self {
        Self
    }

    fn generate_nonce() -> [u8; 8] {
        let mut nonce = [0u8; 8];
        rand::thread_rng().fill_bytes(&mut nonce);
        nonce
    }

    fn compute_hmac(data: &[u8], key: &[u8]) -> [u8; 32] {
        let mut hmac_key = [0u8; 64];
        if key.len() <= 64 {
            hmac_key[..key.len()].copy_from_slice(key);
        } else {
            let hash = digest::digest(&digest::SHA256, key);
            hmac_key[..32].copy_from_slice(hash.as_ref());
        }

        let ipad = hmac_key.iter().map(|&b| b ^ 0x36).collect::<Vec<u8>>();
        let opad = hmac_key.iter().map(|&b| b ^ 0x5c).collect::<Vec<u8>>();

        let mut inner_data = ipad;
        inner_data.extend_from_slice(data);
        let inner_hash = digest::digest(&digest::SHA256, &inner_data);

        let mut outer_data = opad;
        outer_data.extend_from_slice(inner_hash.as_ref());
        let outer_hash = digest::digest(&digest::SHA256, &outer_data);

        let mut result = [0u8; 32];
        result.copy_from_slice(outer_hash.as_ref());
        result
    }
}

#[async_trait]
impl Encryptor for Salsa20Encryptor {
    async fn encrypt(&self, data: &[u8], key: &[u8]) -> AuroraResult<Vec<u8>> {
        if key.len() != 32 {
            return Err(CryptoError::InvalidKey.into());
        }

        let nonce_bytes = Self::generate_nonce();
        let mut cipher = Salsa20::new(key.into(), &nonce_bytes.into());
        
        let mut ciphertext = data.to_vec();
        cipher.apply_keystream(&mut ciphertext);
        
        // Compute HMAC for authentication
        let mut authenticated_data = Vec::new();
        authenticated_data.extend_from_slice(&nonce_bytes);
        authenticated_data.extend_from_slice(&ciphertext);
        let hmac = Self::compute_hmac(&authenticated_data, key);
        
        // Format: nonce (8 bytes) + ciphertext + hmac (32 bytes)
        let mut result = Vec::with_capacity(nonce_bytes.len() + ciphertext.len() + 32);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        result.extend_from_slice(&hmac);
        
        Ok(result)
    }

    async fn decrypt(&self, encrypted_data: &[u8], key: &[u8]) -> AuroraResult<Vec<u8>> {
        if key.len() != 32 {
            return Err(CryptoError::InvalidKey.into());
        }

        if encrypted_data.len() < 40 { // 8 (nonce) + 32 (hmac) = 40 minimum
            return Err(CryptoError::Decryption.into());
        }

        let data_len = encrypted_data.len();
        let (nonce_and_ciphertext, received_hmac) = encrypted_data.split_at(data_len - 32);
        let (nonce_bytes, ciphertext) = nonce_and_ciphertext.split_at(8);
        
        // Verify HMAC
        let computed_hmac = Self::compute_hmac(nonce_and_ciphertext, key);
        if computed_hmac != received_hmac {
            return Err(CryptoError::Decryption.into());
        }
        
        let mut cipher = Salsa20::new(key.into(), nonce_bytes.into());
        let mut plaintext = ciphertext.to_vec();
        cipher.apply_keystream(&mut plaintext);
        
        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_salsa20_encryption_decryption() {
        let encryptor = Salsa20Encryptor::new();
        let key = [0u8; 32]; // Test key
        let data = b"Hello, Salsa20!";

        let encrypted = encryptor.encrypt(data, &key).await.unwrap();
        let decrypted = encryptor.decrypt(&encrypted, &key).await.unwrap();

        assert_eq!(data, decrypted.as_slice());
    }
}