use async_trait::async_trait;
use crate::error::AuroraResult;

/// Trait for encryption operations
#[async_trait]
pub trait Encryptor: Send + Sync {
    async fn encrypt(&self, data: &[u8], key: &[u8]) -> AuroraResult<Vec<u8>>;
    async fn decrypt(&self, encrypted_data: &[u8], key: &[u8]) -> AuroraResult<Vec<u8>>;
}

/// Trait for key exchange operations
#[async_trait]
pub trait KeyExchange: Send + Sync {
    async fn generate_keypair(&self) -> AuroraResult<(Vec<u8>, Vec<u8>)>; // (public, private)
    async fn derive_shared_secret(&self, private_key: &[u8], public_key: &[u8]) -> AuroraResult<Vec<u8>>;
}

/// Trait for digital signatures
#[async_trait]
pub trait DigitalSigner: Send + Sync {
    async fn sign(&self, data: &[u8], private_key: &[u8]) -> AuroraResult<Vec<u8>>;
    async fn verify(&self, data: &[u8], signature: &[u8], public_key: &[u8]) -> AuroraResult<bool>;
}