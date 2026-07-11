use std::io;
use std::net::{TcpListener, ToSocketAddrs};

use super::connection::Connection;

/// Blocking accept loop — v0.3 chưa cần async runtime. Mỗi `accept()` trả
/// về 1 `Connection` độc lập; điều phối nhiều connection cùng lúc (thread
/// per connection hay khác) là việc của caller, không phải của Server.
pub struct Server {
    listener: TcpListener,
}

impl Server {
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        Ok(Self { listener })
    }

    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    pub fn accept(&self) -> io::Result<Connection> {
        let (stream, _addr) = self.listener.accept()?;
        Ok(Connection::new(stream))
    }
}
