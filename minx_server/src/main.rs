#[macro_use]
extern crate log;

use anyhow::Result;
use std::collections::HashMap;
use std::net;
use quiche;
use mio;
use ring::rand;

const MAX_DATAGRAM_SIZE: usize = 1350;

struct PartialResponse {
	body: Vec<u8>,
	written: usize,
}

struct Client {
	conn: std::pin::Pin<Box<quiche::Connection>>,
	http3_conn: Option<quiche::h3::Connection>,
	partial_responses: HashMap<u64, PartialResponse>,
}

type ClientMap = HashMap<Vec<u8>, (net::SocketAddr, Client)>;

use std::{thread, time};
use std::future;

//thread::sleep (time::Duration::from_millis (10));
//_future.poll ();

async fn print_async2 () {
	println! ("print_async2");
}

async fn print_async () -> thread::ThreadId {
	let a = thread::current ().id ();
	println! ("print_async begin");
	print_async2 ().await;
	println! ("print_async end");
	a
}

use async_std::{
    fs::File, // 支持异步操作的文件结构体
    task, // 调用调度器
    prelude::* // Future或输入输出流
};

fn main() {
    let _run = print_async ();
    let _run2 = print_async ();
	let _run = task::spawn (_run); //  task::spawn    task::block_on
	let _run2 = task::spawn (_run2);
	thread::sleep (time::Duration::from_millis (10));
	println! ("main end");
	let _id = task::block_on (_run);
	let _id2 = task::block_on (_run2);
	if _id == _id2 {
		println! ("thread id is equal {}", _id);
	} else {
		println! ("thread id is not equal");
	}
}



fn main2 () -> Result<()> {
	let mut buf = [0; 65535];
	let mut out = [0; MAX_DATAGRAM_SIZE];

	env_logger::builder ().default_format_timestamp_nanos (true).init ();

	let max_data: u64 = 10000000;
	let max_stream_data: u64 = 1000000;
	let max_streams_bidi: u64 = 100;
	let max_streams_uni: u64 = 100;

	let poll = mio::Poll::new ()?;
	let mut events = mio::Events::with_capacity (1024);

	let _local_addr = net::SocketAddr::new (net::IpAddr::V4 (net::Ipv4Addr::new (0, 0, 0, 0)), 5656);
	let socket = net::UdpSocket::bind (_local_addr)?;
	let socket = mio::net::UdpSocket::from_socket (socket)?;
	poll.register (&socket, mio::Token (0), mio::Ready::readable (), mio::PollOpt::edge ())?;

	let mut config = quiche::Config::new (quiche::PROTOCOL_VERSION)?;
	config.load_cert_chain_from_pem_file ("cert.crt")?;
	config.load_priv_key_from_pem_file ("cert.key")?;
	config.set_application_protos (quiche::h3::APPLICATION_PROTOCOL)?;
	config.set_max_idle_timeout (5000);
	config.set_max_packet_size (MAX_DATAGRAM_SIZE as u64);
	config.set_initial_max_data (max_data);
	config.set_initial_max_stream_data_bidi_local (max_stream_data);
	config.set_initial_max_stream_data_bidi_remote (max_stream_data);
	config.set_initial_max_stream_data_uni (max_stream_data);
	config.set_initial_max_streams_bidi (max_streams_bidi);
	config.set_initial_max_streams_uni (max_streams_uni);
	config.set_disable_active_migration (true);
	//config.enable_early_data ();
	//config.grease (false);
	//config.log_keys ();

	config.set_cc_algorithm_name ("reno")?;

	let h3_config = quiche::h3::Config::new ()?;

	let rng = rand::SystemRandom::new ();
	let conn_id_seed = ring::hmac::Key::generate (ring::hmac::HMAC_SHA256, &rng).unwrap ();
	let mut clients = ClientMap::new ();

	loop {
		let timeout = clients.values ().filter_map (|(_, c)| c.conn.timeout ()).min ();
		poll.poll (&mut events, timeout)?;

		'read: loop {
			if events.is_empty () {
				debug! ("timed out");
				clients.values_mut ().for_each (|(_, c)| c.conn.on_timeout ());
				break 'read;
			}

			let (len, src) = match socket.recv_from(&mut buf) {
				Ok (v) => v,
				Err (e) => {
					if e.kind () == std::io::ErrorKind::WouldBlock {
						debug! ("recv() would block");
						break 'read;
					}
					panic! ("recv() failed: {:?}", e);
				},
			};
			debug! ("got {} bytes", len);
			let pkt_buf = &mut buf[..len];
			let hdr = match quiche::Header::from_slice (pkt_buf, quiche::MAX_CONN_ID_LEN) {
				Ok (v) => v,
				Err (e) => {
					error! ("Parsing packet header failed: {:?}", e);
					continue;
				},
			};
			trace! ("got packet {:?}", hdr);
			let conn_id = ring::hmac::sign (&conn_id_seed, &hdr.dcid);
			let conn_id = &conn_id.as_ref ()[..quiche::MAX_CONN_ID_LEN];

			let (_, client) = if !clients.contains_key (&hdr.dcid) && !clients.contains_key (conn_id) {
				if hdr.ty != quiche::Type::Initial {
					error! ("Packet is not Initial");
					continue;
				}

				if !quiche::version_is_supported (hdr.version) {
					warn! ("Doing version negotiation");
					let len = quiche::negotiate_version (&hdr.scid, &hdr.dcid, &mut out)?;
					let out = &out[..len];
					if let Err (e) = socket.send_to (out, &src) {
						if e.kind () == std::io::ErrorKind::WouldBlock {
							debug! ("send () would block");
							break;
						}
						panic! ("send () failed: {:?}", e);
					}
					continue;
				}

				let mut scid = [0; quiche::MAX_CONN_ID_LEN];
				scid.copy_from_slice (&conn_id);

				let mut odcid = None;

				// 无状态的重试，如果需禁用则改为false
				if true {
					let token = hdr.token.as_ref ().unwrap ();
					if token.is_empty () {
						warn! ("Doing stateless retry");
						let new_token = mint_token (&hdr, &src);
						let len = quiche::retry (&hdr.scid, &hdr.dcid, &scid, &new_token, &mut out)?;
						let out = &out[..len];
						if let Err (e) = socket.send_to (out, &src) {
							if e.kind () == std::io::ErrorKind::WouldBlock {
								debug! ("send () would block");
								break;
							}
							panic! ("send () failed: {:?}", e);
						}
						continue;
					}
					odcid = validate_token (&src, token);
					if odcid == None {
						error! ("Invalid address validation token");
						continue;
					}
					if scid.len () != hdr.dcid.len () {
						error! ("Invalid destination connection ID");
						continue;
					}
					scid.copy_from_slice (&hdr.dcid);
				}

				debug! ("New connection: dcid={} scid={}", hex_dump (&hdr.dcid), hex_dump (&scid));
				let conn = quiche::accept (&scid, odcid, &mut config)?;
				let client = Client {
					conn,
					http3_conn: None,
					partial_responses: HashMap::new (),
				};
				clients.insert (scid.to_vec (), (src, client));
				clients.get_mut (&scid[..]).unwrap ()
			} else {
				match clients.get_mut (&hdr.dcid) {
					Some (v) => v,
					None => clients.get_mut (conn_id).unwrap (),
				}
			};

			let read = match client.conn.recv (pkt_buf) {
				Ok (v) => v,
				Err (quiche::Error::Done) => {
					debug! ("{} done reading", client.conn.trace_id ());
					break;
				},
				Err (e) => {
					error! ("{} recv failed: {:?}", client.conn.trace_id (), e);
					break 'read;
				},
			};
			debug! ("{} processed {} bytes", client.conn.trace_id (), read);

			if (client.conn.is_in_early_data () || client.conn.is_established ()) && client.http3_conn.is_none () {
				debug! ("{} QUIC handshake completed, now trying HTTP/3", client.conn.trace_id ());
				let h3_conn = match quiche::h3::Connection::with_transport (&mut client.conn, &h3_config) {
					Ok (v) => v,
					Err (e) => {
						error! ("failed to create HTTP/3 connection: {}", e);
						break 'read;
					},
				};
				// TODO: sanity check h3 connection before adding to map
				client.http3_conn = Some (h3_conn);
			}

			if client.http3_conn.is_some () {
				for stream_id in client.conn.writable () {
					handle_writable (client, stream_id);
				}
				loop {
					let http3_conn = client.http3_conn.as_mut ().unwrap ();
					match http3_conn.poll (&mut client.conn) {
						Ok ((stream_id, quiche::h3::Event::Headers { list, .. })) => {
							handle_request (client, stream_id, &list, "wwwroot/");
						},
						Ok ((stream_id, quiche::h3::Event::Data)) => {
							info! ("{} got data on stream id {}", client.conn.trace_id (), stream_id);
						},
						Ok ((_stream_id, quiche::h3::Event::Finished)) => (),
						Err (quiche::h3::Error::Done) => {
							break;
						},
						Err (e) => {
							error! ("{} HTTP/3 error {:?}", client.conn.trace_id (), e);
							break 'read;
						},
					}
				}
			}
		}

		// 发送所有包
		for (peer, client) in clients.values_mut () {
			loop {
				let write = match client.conn.send (&mut out) {
					Ok (v) => v,
					Err (quiche::Error::Done) => {
						debug! ("{} done writing", client.conn.trace_id ());
						break;
					},
					Err (e) => {
						error! ("{} send failed: {:?}", client.conn.trace_id (), e);
						client.conn.close (false, 0x1, b"fail").ok ();
						break;
					},
				};

				// TODO: coalesce packets.
				if let Err (e) = socket.send_to (&out[..write], &peer) {
					if e.kind () == std::io::ErrorKind::WouldBlock {
						debug! ("send () would block");
						break;
					}
					panic! ("send () failed: {:?}", e);
				}
				debug! ("{} written {} bytes", client.conn.trace_id (), write);
			}
		}

		// 关闭链接
		clients.retain (|_, (_, ref mut c)| {
			debug! ("Collecting garbage");
			if c.conn.is_closed () {
				info! ("{} connection collected {:?}", c.conn.trace_id (), c.conn.stats ());
			}
			!c.conn.is_closed ()
		});
	}
}

/// Generate a stateless retry token.
///
/// The token includes the static string `"quiche"` followed by the IP address of the client and by the original destination connection ID generated by the client.
///
/// Note that this function is only an example and doesn't do any cryptographic authenticate of the token. *It should not be used in production system*.
fn mint_token (hdr: &quiche::Header, src: &net::SocketAddr) -> Vec<u8> {
	let mut token = Vec::new ();

	token.extend_from_slice (b"quiche");

	let addr = match src.ip () {
		std::net::IpAddr::V4(a) => a.octets ().to_vec (),
		std::net::IpAddr::V6(a) => a.octets ().to_vec (),
	};

	token.extend_from_slice (&addr);
	token.extend_from_slice (&hdr.dcid);

	token
}

/// Validates a stateless retry token.
///
/// This checks that the ticket includes the `"quiche"` static string, and that
/// the client IP address matches the address stored in the ticket.
///
/// Note that this function is only an example and doesn't do any cryptographic
/// authenticate of the token. *It should not be used in production system*.
fn validate_token<'a> (src: &net::SocketAddr, token: &'a [u8]) -> Option<&'a [u8]> {
	if token.len () < 6 {
		return None;
	}
	if &token[..6] != b"quiche" {
		return None;
	}
	let token = &token[6..];
	let addr = match src.ip () {
		std::net::IpAddr::V4(a) => a.octets ().to_vec (),
		std::net::IpAddr::V6(a) => a.octets ().to_vec (),
	};
	if token.len () < addr.len () || &token[..addr.len ()] != addr.as_slice () {
		return None;
	}
	let token = &token[addr.len ()..];
	Some (&token[..])
}

/// Handles incoming HTTP/3 requests.
fn handle_request (client: &mut Client, stream_id: u64, headers: &[quiche::h3::Header], root: &str) {
	let conn = &mut client.conn;
	let http3_conn = &mut client.http3_conn.as_mut ().unwrap ();
	info! ("{} got request {:?} on stream id {}", conn.trace_id (), headers, stream_id);
	conn.stream_shutdown (stream_id, quiche::Shutdown::Read, 0).unwrap ();
	let (headers, body) = build_response (root, headers);
	if let Err (e) = http3_conn.send_response (conn, stream_id, &headers, false) {
		error! ("{} stream send failed {:?}", conn.trace_id (), e);
	}

	let written = match http3_conn.send_body (conn, stream_id, &body, true) {
		Ok (v) => v,
		Err (quiche::h3::Error::Done) => 0,
		Err (e) => {
			error! ("{} stream send failed {:?}", conn.trace_id (), e);
			return;
		},
	};

	if written < body.len () {
		let response = PartialResponse { body, written };
		client.partial_responses.insert (stream_id, response);
	}
}

/// Builds an HTTP/3 response given a request.
fn build_response (root: &str, request: &[quiche::h3::Header]) -> (Vec<quiche::h3::Header>, Vec<u8>) {
	let mut file_path = std::path::PathBuf::from (root);
	let mut path = std::path::Path::new ("");
	let mut method = "";

	// Look for the request's path and method.
	for hdr in request {
		match hdr.name () {
			":path" => {
				path = std::path::Path::new (hdr.value ());
			},
			":method" => {
				method = hdr.value ();
			},
			_ => (),
		}
	}

	let (status, body) = match method {
		"GET" => {
			for c in path.components () {
				if let std::path::Component::Normal (v) = c {
					file_path.push (v)
				}
			}
			match std::fs::read (file_path.as_path ()) {
				Ok (data) => (200, data),
				Err (_) => (404, b"Not Found!".to_vec ()),
			}
		},

		_ => (405, Vec::new ()),
	};

	let headers = vec![
		quiche::h3::Header::new (":status", &status.to_string ()),
		quiche::h3::Header::new ("server", "quiche"),
		quiche::h3::Header::new ("content-length", &body.len ().to_string ()),
	];

	(headers, body)
}

/// Handles newly writable streams.
fn handle_writable (client: &mut Client, stream_id: u64) {
	let conn = &mut client.conn;
	let http3_conn = &mut client.http3_conn.as_mut ().unwrap ();

	debug! ("{} stream {} is writable", conn.trace_id (), stream_id);
	if !client.partial_responses.contains_key (&stream_id) {
		return;
	}
	let resp = client.partial_responses.get_mut (&stream_id).unwrap ();
	let body = &resp.body[resp.written..];

	let written = match http3_conn.send_body (conn, stream_id, body, true) {
		Ok (v) => v,
		Err (quiche::h3::Error::Done) => 0,
		Err (e) => {
			error! ("{} stream send failed {:?}", conn.trace_id (), e);
			return;
		},
	};
	resp.written += written;
	if resp.written == resp.body.len () {
		client.partial_responses.remove (&stream_id);
	}
}

fn hex_dump (buf: &[u8]) -> String {
	let vec: Vec<String> = buf.iter ().map (|b| format! ("{:02x}", b)).collect ();
	vec.join ("")
}
