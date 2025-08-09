use async_trait::async_trait;
use rsa::{RsaPrivateKey, RsaPublicKey, Pkcs1v15Encrypt};
use rsa::pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey, DecodeRsaPrivateKey, DecodeRsaPublicKey};
use rand::rngs::OsRng;

use crate::error::{AuroraResult, CryptoError};
use super::traits::KeyExchange;

pub struct RsaKeyExchange;

impl RsaKeyExchange {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl KeyExchange for RsaKeyExchange {
    async fn generate_keypair(&self) -> AuroraResult<(Vec<u8>, Vec<u8>)> {
        let mut rng = OsRng;
        
        let private_key = RsaPrivateKey::new(&mut rng, 2048)
            .map_err(|_| CryptoError::KeyGeneration)?;
        
        let public_key = RsaPublicKey::from(&private_key);
        
        let private_pem = private_key.to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)
            .map_err(|_| CryptoError::KeyGeneration)?;
        
        let public_pem = public_key.to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)
            .map_err(|_| CryptoError::KeyGeneration)?;
        
        Ok((public_pem.as_bytes().to_vec(), private_pem.as_bytes().to_vec()))
    }

    async fn derive_shared_secret(&self, private_key: &[u8], public_key: &[u8]) -> AuroraResult<Vec<u8>> {
        let private_key_str = std::str::from_utf8(private_key)
            .map_err(|_| CryptoError::InvalidKey)?;
        let public_key_str = std::str::from_utf8(public_key)
            .map_err(|_| CryptoError::InvalidKey)?;

        let private_key = RsaPrivateKey::from_pkcs1_pem(private_key_str)
            .map_err(|_| CryptoError::InvalidKey)?;
        let public_key = RsaPublicKey::from_pkcs1_pem(public_key_str)
            .map_err(|_| CryptoError::InvalidKey)?;

        // Generate a random shared secret and encrypt it with the public key
        let mut shared_secret = [0u8; 32];
        rand::RngCore::fill_bytes(&mut OsRng, &mut shared_secret);
        
        let encrypted_secret = public_key.encrypt(&mut OsRng, Pkcs1v15Encrypt, &shared_secret)
            .map_err(|_| CryptoError::KeyExchange)?;
        
        Ok(encrypted_secret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rsa_key_generation() {
        let key_exchange = RsaKeyExchange::new();
        let (public_key, private_key) = key_exchange.generate_keypair().await.unwrap();
        
        assert!(!public_key.is_empty());
        assert!(!private_key.is_empty());
        assert!(public_key.len() > 100);
        assert!(private_key.len() > 100);
    }
}