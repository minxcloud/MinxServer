use mio::net::UdpSocket;
use mio::{Events, Interest, Poll, Token};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
//use std::rc::*;
use std::sync::*;

pub struct udp_server_t {
	m_socket: Weak<UdpSocket>,
}

impl socket_server_t for udp_server_t {
	async fn run (&mut self) -> io::Result<()> {
		let _addr = SocketAddr::new (IpAddr::V4 (Ipv4Addr::new (0, 0, 0, 0)), _port);
		let _socket_raw = UdpSocket::bind (_addr)?;
		let _socket = Arc::new (_socket_raw);
		////let _a = Arc::new (SocketAddr::new (IpAddr::V4 (Ipv4Addr::new (0, 0, 0, 0)), _port));
		////let _b = Arc::downgrade (&_a);
		////let _c = _b.upgrade ();
		//
		//let mut _poll = Poll::new ()?;
		//let _token = Token (0);
		//let _socket_copy = self.m_socket.clone ();
		//let mut _socket_shadow = &*_socket_copy;
		//_poll.registry ().register (&mut _socket_shadow, _token, Interest::READABLE)?;
		//self.m_socket = Some (_socket_shadow);
		//let mut _buf = [0; 1 << 16];
		//let mut _events = Events::with_capacity (1);
		//loop {
		//	_poll.poll (&mut _events, None)?;
		//	for event in _events.iter () {
		//		match event.token () {
		//			_token => loop {
		//				match _socket_shadow.recv_from (&mut _buf) {
		//					Ok ((packet_size, source_address)) => {
		//						// Echo the data.
		//						_socket_shadow.send_to (&_buf[..packet_size], source_address)?;
		//					},
		//					//Err (e) if e.kind () == io::ErrorKind::WouldBlock => { break; },
		//					Err (e) => {
		//						if e.kind () == io::ErrorKind::WouldBlock {
		//							break;
		//						} else {
		//							//Err (e);
		//							return;
		//						}
		//					}
		//				}
		//			},
		//			_ => {
		//				warn! ("Got event for unexpected token: {:?}", _events);
		//			}
		//		}
		//	}
		//}
	}

	fn send_data (&mut self, _addr: SocketAddr, &_buf: [u8]) -> io::Result<()> {
		let _socket_copy = self.m_socket.clone ();
		let mut _socket_shadow = &*_socket_copy;
		_socket_shadow.send_to (&_buf[..], _addr)?;
		Ok (())
	}
}

impl udp_server_t {
	pub fn new (_port: u16) -> udp_server_t {
		udp_server_t {
			m_socket: Weak::default (),
		}
	}
}
