use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, error};

use crate::error::{AuroraResult, NetworkError};

pub struct Socks5Proxy {
    bind_addr: SocketAddr,
}

impl Socks5Proxy {
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self { bind_addr }
    }

    pub async fn start(&self) -> AuroraResult<()> {
        let listener = TcpListener::bind(self.bind_addr).await
            .map_err(|_| NetworkError::ProxyConfig)?;

        info!("SOCKS5 proxy listening on {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New SOCKS5 connection from {}", addr);
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream).await {
                            error!("SOCKS5 connection error: {:?}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept SOCKS5 connection: {}", e);
                }
            }
        }
    }

    async fn handle_connection(mut stream: TcpStream) -> AuroraResult<()> {
        // SOCKS5 handshake
        let mut buffer = [0u8; 1024];
        
        // Read initial request
        let n = stream.read(&mut buffer).await
            .map_err(|_| NetworkError::Transport("Failed to read from client".to_string()))?;

        if n < 3 || buffer[0] != 0x05 {
            return Err(NetworkError::Transport("Invalid SOCKS5 request".to_string()).into());
        }

        // Send authentication method (no auth)
        stream.write_all(&[0x05, 0x00]).await
            .map_err(|_| NetworkError::Transport("Failed to write to client".to_string()))?;

        // Read connection request
        let n = stream.read(&mut buffer).await
            .map_err(|_| NetworkError::Transport("Failed to read connection request".to_string()))?;

        if n < 10 || buffer[0] != 0x05 || buffer[1] != 0x01 {
            return Err(NetworkError::Transport("Invalid SOCKS5 connection request".to_string()).into());
        }

        // Parse target address
        let (target_addr, target_port) = Self::parse_target_address(&buffer[3..n])?;
        
        info!("SOCKS5 connecting to {}:{}", target_addr, target_port);

        // Connect to target
        match TcpStream::connect(format!("{}:{}", target_addr, target_port)).await {
            Ok(target_stream) => {
                // Send success response
                stream.write_all(&[0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).await
                    .map_err(|_| NetworkError::Transport("Failed to send success response".to_string()))?;

                // Start proxying data
                Self::proxy_data(stream, target_stream).await?;
            }
            Err(_) => {
                // Send connection refused response
                stream.write_all(&[0x05, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).await
                    .map_err(|_| NetworkError::Transport("Failed to send error response".to_string()))?;
            }
        }

        Ok(())
    }

    fn parse_target_address(data: &[u8]) -> AuroraResult<(String, u16)> {
        if data.is_empty() {
            return Err(NetworkError::Transport("Empty address data".to_string()).into());
        }

        match data[0] {
            0x01 => {
                // IPv4
                if data.len() < 7 {
                    return Err(NetworkError::Transport("Invalid IPv4 address".to_string()).into());
                }
                let addr = format!("{}.{}.{}.{}", data[1], data[2], data[3], data[4]);
                let port = u16::from_be_bytes([data[5], data[6]]);
                Ok((addr, port))
            }
            0x03 => {
                // Domain name
                if data.len() < 2 {
                    return Err(NetworkError::Transport("Invalid domain name".to_string()).into());
                }
                let len = data[1] as usize;
                if data.len() < len + 4 {
                    return Err(NetworkError::Transport("Invalid domain name length".to_string()).into());
                }
                let domain = String::from_utf8_lossy(&data[2..2 + len]).to_string();
                let port = u16::from_be_bytes([data[2 + len], data[3 + len]]);
                Ok((domain, port))
            }
            _ => Err(NetworkError::Transport("Unsupported address type".to_string()).into()),
        }
    }

    async fn proxy_data(client: TcpStream, target: TcpStream) -> AuroraResult<()> {
        let (mut client_read, mut client_write) = client.into_split();
        let (mut target_read, mut target_write) = target.into_split();

        let client_to_target = async {
            tokio::io::copy(&mut client_read, &mut target_write).await
        };

        let target_to_client = async {
            tokio::io::copy(&mut target_read, &mut client_write).await
        };

        tokio::select! {
            result = client_to_target => {
                if let Err(e) = result {
                    error!("Client to target proxy error: {}", e);
                }
            }
            result = target_to_client => {
                if let Err(e) = result {
                    error!("Target to client proxy error: {}", e);
                }
            }
        }

        Ok(())
    }
}