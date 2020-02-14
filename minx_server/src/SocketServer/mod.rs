use std::io;
use std::net::SocketAddr;

mod udp_server;
use udp_server::*;

pub trait socket_server_t {
	fn run (&mut self) -> io::Result<()>;
	fn send_data (&mut self, _addr: SocketAddr, &_buf: [u8]) -> io::Result<()>;
}

pub fn socket_server_new (_port: u16) -> io::Result<Box<dyn socket_server_t>> {
	udp_server_t::new (_port)
}
