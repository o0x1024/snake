use std::sync::Arc;
use crate::error::{AuroraResult, CryptoError};
use super::traits::Encryptor;
use super::encryption::AesGcmEncryptor;
use super::chacha20::ChaCha20Poly1305Encryptor;
use super::salsa20::Salsa20Encryptor;
use base64::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    None,
    AesGcm,
    ChaCha20Poly1305,
    Salsa20,
}

impl EncryptionAlgorithm {
    pub fn from_str(s: &str) -> AuroraResult<Self> {
        match s.to_lowercase().as_str() {
            "none" => Ok(Self::None),
            "aes-256-gcm" | "aes_gcm" | "aesgcm" => Ok(Self::AesGcm),
            "chacha20-poly1305" | "chacha20_poly1305" | "chacha20poly1305" => Ok(Self::ChaCha20Poly1305),
            "salsa20" => Ok(Self::Salsa20),
            _ => Err(CryptoError::UnsupportedAlgorithm(s.to_string()).into()),
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::AesGcm => "aes-256-gcm",
            Self::ChaCha20Poly1305 => "chacha20-poly1305",
            Self::Salsa20 => "salsa20",
        }
    }
}

pub struct EncryptorFactory;

impl EncryptorFactory {
    pub fn create_encryptor(algorithm: &EncryptionAlgorithm) -> AuroraResult<Option<Arc<dyn Encryptor>>> {
        match algorithm {
            EncryptionAlgorithm::None => Ok(None),
            EncryptionAlgorithm::AesGcm => Ok(Some(Arc::new(AesGcmEncryptor::new()))),
            EncryptionAlgorithm::ChaCha20Poly1305 => Ok(Some(Arc::new(ChaCha20Poly1305Encryptor::new()))),
            EncryptionAlgorithm::Salsa20 => Ok(Some(Arc::new(Salsa20Encryptor::new()))),
        }
    }

    pub fn get_supported_algorithms() -> Vec<&'static str> {
        vec!["none", "aes-256-gcm", "chacha20-poly1305", "salsa20"]
    }
}

/// 加密通信协议的数据结构
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct EncryptedRequest {
    pub encrypted_data: String, // base64编码的加密数据
    pub nonce: Option<String>,  // base64编码的nonce（某些算法需要）
    pub algorithm: String,      // 加密算法名称
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct CommandPayload {
    pub cmd: String,
    pub timestamp: i64,
}

/// 加密工具类
pub struct CryptoUtils;

impl CryptoUtils {
    /// 加密命令载荷
    pub async fn encrypt_command(
        cmd: &str,
        algorithm: &EncryptionAlgorithm,
        key: &[u8],
    ) -> AuroraResult<EncryptedRequest> {
        let payload = CommandPayload {
            cmd: cmd.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        let payload_json = serde_json::to_string(&payload)
            .map_err(|_| CryptoError::Serialization)?;
        
        match EncryptorFactory::create_encryptor(algorithm)? {
            Some(encryptor) => {
                let encrypted_data = encryptor.encrypt(payload_json.as_bytes(), key).await?;
                Ok(EncryptedRequest {
                    encrypted_data: BASE64_STANDARD.encode(&encrypted_data),
                    nonce: None, // nonce已包含在encrypted_data中
                    algorithm: algorithm.to_string().to_string(),
                })
            }
            None => {
                // 无加密
                Ok(EncryptedRequest {
                    encrypted_data: BASE64_STANDARD.encode(payload_json.as_bytes()),
                    nonce: None,
                    algorithm: algorithm.to_string().to_string(),
                })
            }
        }
    }

    /// 解密响应数据
    pub async fn decrypt_response(
        encrypted_response: &str,
        algorithm: &EncryptionAlgorithm,
        key: &[u8],
    ) -> AuroraResult<String> {
        let encrypted_data = BASE64_STANDARD.decode(encrypted_response)
            .map_err(|_| CryptoError::Decryption)?;
        
        match EncryptorFactory::create_encryptor(algorithm)? {
            Some(encryptor) => {
                // 首先尝试标准格式解密
                match encryptor.decrypt(&encrypted_data, key).await {
                    Ok(decrypted_data) => {
                        String::from_utf8(decrypted_data)
                            .map_err(|_| CryptoError::Decryption.into())
                    }
                    Err(_) => {
                        // 如果标准格式失败，尝试纯PHP格式（AES-CBC+HMAC）
                        if *algorithm == EncryptionAlgorithm::AesGcm {
                            Self::decrypt_pure_php_format(&encrypted_data, key).await
                        } else {
                            Err(CryptoError::Decryption.into())
                        }
                    }
                }
            }
            None => {
                // 无加密
                String::from_utf8(encrypted_data)
                    .map_err(|_| CryptoError::Decryption.into())
            }
        }
    }
    
    /// 解密纯PHP格式的数据（AES-CBC+HMAC）
    async fn decrypt_pure_php_format(
        encrypted_data: &[u8],
        key: &[u8],
    ) -> AuroraResult<String> {
        use aes::Aes256;
        use cbc::{Decryptor, cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit}};
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        
        type HmacSha256 = Hmac<Sha256>;
        type Aes256CbcDec = cbc::Decryptor<Aes256>;
        
        // 纯PHP格式: IV(16) + HMAC(32) + Ciphertext
        if encrypted_data.len() < 48 {
            return Err(CryptoError::Decryption.into());
        }
        
        let iv = &encrypted_data[0..16];
        let received_hmac = &encrypted_data[16..48];
        let ciphertext = &encrypted_data[48..];
        
        // 验证HMAC
        let mut mac = HmacSha256::new_from_slice(key)
            .map_err(|_| CryptoError::Decryption)?;
        mac.update(iv);
        mac.update(ciphertext);
        
        mac.verify_slice(received_hmac)
            .map_err(|_| CryptoError::Decryption)?;
        
        // 解密数据
        let cipher = Aes256CbcDec::new_from_slices(key, iv)
            .map_err(|_| CryptoError::Decryption)?;
        
        let mut buffer = ciphertext.to_vec();
        let decrypted = cipher.decrypt_padded_mut::<Pkcs7>(&mut buffer)
            .map_err(|_| CryptoError::Decryption)?;
        
        String::from_utf8(decrypted.to_vec())
            .map_err(|_| CryptoError::Decryption.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_encryption_factory() {
        let algorithms = vec![
            EncryptionAlgorithm::AesGcm,
            EncryptionAlgorithm::ChaCha20Poly1305,
            EncryptionAlgorithm::Salsa20,
        ];

        for algorithm in algorithms {
            let encryptor = EncryptorFactory::create_encryptor(&algorithm).unwrap();
            assert!(encryptor.is_some());
        }
    }

    #[tokio::test]
    async fn test_crypto_utils() {
        let key = [0u8; 32];
        let cmd = "ls -la";
        
        for algorithm_str in ["aes-256-gcm", "chacha20-poly1305", "salsa20"] {
            let algorithm = EncryptionAlgorithm::from_str(algorithm_str).unwrap();
            
            let encrypted_request = CryptoUtils::encrypt_command(cmd, &algorithm, &key).await.unwrap();
            assert_eq!(encrypted_request.algorithm, algorithm_str);
            
            // 模拟响应解密
            let response = "command output";
            let encrypted_response = if algorithm != EncryptionAlgorithm::None {
                let encryptor = EncryptorFactory::create_encryptor(&algorithm).unwrap().unwrap();
                let encrypted = encryptor.encrypt(response.as_bytes(), &key).await.unwrap();
                BASE64_STANDARD.encode(&encrypted)
            } else {
                BASE64_STANDARD.encode(response.as_bytes())
            };
            
            let decrypted = CryptoUtils::decrypt_response(&encrypted_response, &algorithm, &key).await.unwrap();
            assert_eq!(decrypted, response);
        }
    }
}