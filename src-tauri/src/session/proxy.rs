use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{AuroraResult, NetworkError};
use super::types::{ProxyConfig, ProxyType};

pub struct ProxyConnector {
    config: ProxyConfig,
}

impl ProxyConnector {
    pub fn new(config: ProxyConfig) -> Self {
        Self { config }
    }

    pub async fn connect<A: ToSocketAddrs>(&self, target: A) -> AuroraResult<TcpStream> {
        match self.config.proxy_type {
            ProxyType::Socks5 => self.connect_socks5(target).await,
            ProxyType::Http | ProxyType::Https => {
                // For now, we'll focus on SOCKS5 as specified in the requirements
                Err(NetworkError::ProxyConfig.into())
            }
        }
    }

    async fn connect_socks5<A: ToSocketAddrs>(&self, target: A) -> AuroraResult<TcpStream> {
        // Connect to SOCKS5 proxy
        let mut stream = tokio::time::timeout(
            Duration::from_secs(10),
            TcpStream::connect(self.config.address)
        )
        .await
        .map_err(|_| NetworkError::ConnectionFailed)?
        .map_err(|_| NetworkError::ConnectionFailed)?;

        // SOCKS5 authentication negotiation
        if let Some(username) = &self.config.username {
            self.socks5_auth(&mut stream, username, self.config.password.as_deref().unwrap_or("")).await?;
        } else {
            self.socks5_no_auth(&mut stream).await?;
        }

        // SOCKS5 connection request
        let target_addr = self.resolve_target_addr(target).await?;
        self.socks5_connect(&mut stream, &target_addr).await?;

        Ok(stream)
    }

    async fn socks5_no_auth(&self, stream: &mut TcpStream) -> AuroraResult<()> {
        // Send authentication method negotiation
        let auth_request = [0x05, 0x01, 0x00]; // Version 5, 1 method, no authentication
        stream.write_all(&auth_request).await?;

        // Read server response
        let mut response = [0u8; 2];
        stream.read_exact(&mut response).await?;

        if response[0] != 0x05 {
            return Err(NetworkError::ProxyConfig.into());
        }

        if response[1] != 0x00 {
            return Err(NetworkError::ProxyConfig.into());
        }

        Ok(())
    }

    async fn socks5_auth(&self, stream: &mut TcpStream, username: &str, password: &str) -> AuroraResult<()> {
        // Send authentication method negotiation
        let auth_request = [0x05, 0x01, 0x02]; // Version 5, 1 method, username/password
        stream.write_all(&auth_request).await?;

        // Read server response
        let mut response = [0u8; 2];
        stream.read_exact(&mut response).await?;

        if response[0] != 0x05 || response[1] != 0x02 {
            return Err(NetworkError::ProxyConfig.into());
        }

        // Send username/password authentication
        let mut auth_data = Vec::new();
        auth_data.push(0x01); // Version
        auth_data.push(username.len() as u8);
        auth_data.extend_from_slice(username.as_bytes());
        auth_data.push(password.len() as u8);
        auth_data.extend_from_slice(password.as_bytes());

        stream.write_all(&auth_data).await?;

        // Read authentication response
        let mut auth_response = [0u8; 2];
        stream.read_exact(&mut auth_response).await?;

        if auth_response[0] != 0x01 || auth_response[1] != 0x00 {
            return Err(NetworkError::ProxyConfig.into());
        }

        Ok(())
    }

    async fn socks5_connect(&self, stream: &mut TcpStream, target_addr: &TargetAddr) -> AuroraResult<()> {
        let mut connect_request = Vec::new();
        connect_request.extend_from_slice(&[0x05, 0x01, 0x00]); // Version, Connect, Reserved

        match target_addr {
            TargetAddr::Ip(addr) => {
                match addr {
                    SocketAddr::V4(v4) => {
                        connect_request.push(0x01); // IPv4
                        connect_request.extend_from_slice(&v4.ip().octets());
                        connect_request.extend_from_slice(&v4.port().to_be_bytes());
                    }
                    SocketAddr::V6(v6) => {
                        connect_request.push(0x04); // IPv6
                        connect_request.extend_from_slice(&v6.ip().octets());
                        connect_request.extend_from_slice(&v6.port().to_be_bytes());
                    }
                }
            }
            TargetAddr::Domain(domain, port) => {
                connect_request.push(0x03); // Domain name
                connect_request.push(domain.len() as u8);
                connect_request.extend_from_slice(domain.as_bytes());
                connect_request.extend_from_slice(&port.to_be_bytes());
            }
        }

        stream.write_all(&connect_request).await?;

        // Read connection response
        let mut response = [0u8; 4];
        stream.read_exact(&mut response).await?;

        if response[0] != 0x05 {
            return Err(NetworkError::ProxyConfig.into());
        }

        if response[1] != 0x00 {
            return Err(NetworkError::ConnectionFailed.into());
        }

        // Read the bound address (we don't need it, but must consume it)
        match response[3] {
            0x01 => {
                // IPv4
                let mut addr = [0u8; 6]; // 4 bytes IP + 2 bytes port
                stream.read_exact(&mut addr).await?;
            }
            0x03 => {
                // Domain name
                let mut len = [0u8; 1];
                stream.read_exact(&mut len).await?;
                let mut domain = vec![0u8; len[0] as usize + 2]; // domain + 2 bytes port
                stream.read_exact(&mut domain).await?;
            }
            0x04 => {
                // IPv6
                let mut addr = [0u8; 18]; // 16 bytes IP + 2 bytes port
                stream.read_exact(&mut addr).await?;
            }
            _ => return Err(NetworkError::ProxyConfig.into()),
        }

        Ok(())
    }

    async fn resolve_target_addr<A: ToSocketAddrs>(&self, target: A) -> AuroraResult<TargetAddr> {
        // Try to resolve the target address
        let mut addrs = tokio::net::lookup_host(target).await?;
        
        if let Some(addr) = addrs.next() {
            Ok(TargetAddr::Ip(addr))
        } else {
            Err(NetworkError::ConnectionFailed.into())
        }
    }
}

#[derive(Debug, Clone)]
enum TargetAddr {
    Ip(SocketAddr),
    Domain(String, u16),
}

pub struct ProxyTunnel {
    stream: TcpStream,
    proxy_config: ProxyConfig,
}

impl ProxyTunnel {
    pub async fn establish(proxy_config: ProxyConfig, target: SocketAddr) -> AuroraResult<Self> {
        let connector = ProxyConnector::new(proxy_config.clone());
        let stream = connector.connect(target).await?;

        Ok(Self {
            stream,
            proxy_config,
        })
    }

    pub async fn send_data(&mut self, data: &[u8]) -> AuroraResult<()> {
        self.stream.write_all(data).await?;
        Ok(())
    }

    pub async fn receive_data(&mut self, buffer: &mut [u8]) -> AuroraResult<usize> {
        let bytes_read = self.stream.read(buffer).await?;
        Ok(bytes_read)
    }

    pub async fn close(mut self) -> AuroraResult<()> {
        self.stream.shutdown().await?;
        Ok(())
    }

    pub fn get_proxy_config(&self) -> &ProxyConfig {
        &self.proxy_config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn test_proxy_config_creation() {
        let config = ProxyConfig {
            proxy_type: ProxyType::Socks5,
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1080),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
        };

        let connector = ProxyConnector::new(config);
        // This test just verifies the connector can be created
        assert!(true);
    }
}