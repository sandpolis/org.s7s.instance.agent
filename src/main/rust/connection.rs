//============================================================================//
//                                                                            //
//                         Copyright © 2015 Sandpolis                         //
//                                                                            //
//  This source file is subject to the terms of the Mozilla Public License    //
//  version 2. You may not use this file except in compliance with the MPL    //
//  as published by the Mozilla Foundation.                                   //
//                                                                            //
//============================================================================//

use anyhow::Result;
use crate::core::instance::metatypes::*;
use crate::core::net::message::MSG;
use crate::core::net::msg_cvid::*;
use crate::lib::messages::rq;
use log::{debug, info};
use protobuf::Message;
use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use rustls::Session;
use std::sync::Arc;
use rustls;
use webpki;
use webpki_roots;

pub struct CvidHandshakeError;
pub enum MessageSendError {
	ConnectionClosed,
	Other,
}

pub enum MessageRecvError {
	ConnectionClosed,
	Other,
}

pub enum ConnectionState {
	NotConnected,
}

pub struct Connection {

	/// The underlying transport stream
	pub stream: rustls::StreamOwned<rustls::ClientSession, TcpStream>,

	/// The connection state
	pub state: ConnectionState,

	/// The remote SID
	pub sid: Option<i32>,

	/// The remote UUID
	pub uuid: Option<String>,

	receive_map: Mutex<HashMap<i32, MSG>>,
}

impl Connection {

	pub fn send(&mut self, message: &MSG) -> Result<(), MessageSendError> {
		self.stream.write_all(&message.write_to_bytes().unwrap());
		return Ok(())
	}

	pub fn recv(&self, id: i32) -> Result<MSG, MessageRecvError> {

		// First check the receive map
		let mut receive_map = self.receive_map.lock().unwrap();
		if let Some(msg) = receive_map.remove(&id) {
			return Ok(msg);
		}

		// TODO read stream
		return Err(MessageRecvError::Other);
	}

	fn cvid_handshake(&mut self, uuid: String) -> Result<i32, CvidHandshakeError> {

		let operation_start = Instant::now();

		let mut rq_cvid = RQ_Cvid::new();
		rq_cvid.instance = InstanceType::AGENT;
		rq_cvid.instance_flavor = InstanceFlavor::AGENT_MICRO;
		rq_cvid.uuid = uuid;

		let rq = rq(&rq_cvid);

		if let Err(error) = self.send(&rq) {
			// TODO
		}

		if let Ok(rs) = self.recv(rq.id) {
			if let Ok(rs_cvid) = RS_Cvid::parse_from_bytes(&rs.payload) {
				debug!("Completed SID handshake in {:?} ms", operation_start.elapsed());
				debug!("Assigned SID: {}", rs_cvid.sid);
				debug!("Discovered server UUID: {}", rs_cvid.server_uuid);
				debug!("Discovered server SID: {}", rs_cvid.server_cvid);
				return Ok(rs_cvid.sid);
			}
		}

		return Err(CvidHandshakeError);
	}
}

/// Create a new TLS connection to the given server.
pub fn connect(host: &str, port: u16) -> Result<Connection> {

	let mut config = rustls::ClientConfig::new();
	config
		.root_store
		.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

	let dns_name = webpki::DNSNameRef::try_from_ascii_str(host)?;
	let mut session = rustls::ClientSession::new(&Arc::new(config), dns_name);
	let mut tcp_stream = TcpStream::connect(host)?;
	let tls_stream = rustls::StreamOwned::new(session, tcp_stream);

	return Ok(Connection {
		stream: tls_stream,
		state: ConnectionState::NotConnected,
		sid: None,
		uuid: None,
		receive_map: Mutex::new(HashMap::new()),
	});
}
