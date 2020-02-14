use std::io;

mod SocketServer;
use crate::SocketServer::*;

fn main_run () -> io::Result<()> {
	let mut _server = socket_server_new ();
	_server.run ()
}

fn main () -> io::Result<()> {
	println! ("hello");
    env_logger::init ();
    match main_run () {
		Ok (ret) => Ok (ret),
		Err (err) => {
			Err (err)
		}
	}
}
