use std::io;
use std::net::{TcpListener, ToSocketAddrs};

use super::connection::{Connection, ConnectionReader, ConnectionWriter};

/// Blocking accept loop — v0.4 chưa cần async runtime. Mỗi `accept()` trả
/// thẳng `(ConnectionReader, ConnectionWriter)` đã tách sẵn — Cyclone không
/// có API nào trả về 1 connection 2 chiều chưa tách, xem lý do ở
/// `connection::Connection`. Điều phối nhiều connection cùng lúc (thread
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

    pub fn accept(&self) -> io::Result<(ConnectionReader, ConnectionWriter)> {
        let (stream, _addr) = self.listener.accept()?;
        Connection::new(stream).into_split()
    }
}
