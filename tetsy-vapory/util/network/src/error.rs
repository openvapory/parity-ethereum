// Copyright 2015-2020 Parity Technologies (UK) Ltd.
// This file is part of Tetsy Vapory.

// Tetsy Vapory is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Tetsy Vapory is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Tetsy Vapory.  If not, see <http://www.gnu.org/licenses/>.

use std::{error, io, net, fmt};
use libc::{ENFILE, EMFILE};
use io::IoError;
use {tetsy_rlp, crypto, snappy};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DisconnectReason
{
	DisconnectRequested,
	TCPError,
	BadProtocol,
	UselessPeer,
	TooManyPeers,
	DuplicatePeer,
	IncompatibleProtocol,
	NullIdentity,
	ClientQuit,
	UnexpectedIdentity,
	LocalIdentity,
	PingTimeout,
	Unknown,
}

impl DisconnectReason {
	pub fn from_u8(n: u8) -> DisconnectReason {
		match n {
			0 => DisconnectReason::DisconnectRequested,
			1 => DisconnectReason::TCPError,
			2 => DisconnectReason::BadProtocol,
			3 => DisconnectReason::UselessPeer,
			4 => DisconnectReason::TooManyPeers,
			5 => DisconnectReason::DuplicatePeer,
			6 => DisconnectReason::IncompatibleProtocol,
			7 => DisconnectReason::NullIdentity,
			8 => DisconnectReason::ClientQuit,
			9 => DisconnectReason::UnexpectedIdentity,
			10 => DisconnectReason::LocalIdentity,
			11 => DisconnectReason::PingTimeout,
			_ => DisconnectReason::Unknown,
		}
	}
}

impl fmt::Display for DisconnectReason {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		use self::DisconnectReason::*;

		let msg = match *self {
			DisconnectRequested => "disconnect requested",
			TCPError => "TCP error",
			BadProtocol => "bad protocol",
			UselessPeer => "useless peer",
			TooManyPeers => "too many peers",
			DuplicatePeer => "duplicate peer",
			IncompatibleProtocol => "incompatible protocol",
			NullIdentity => "null identity",
			ClientQuit => "client quit",
			UnexpectedIdentity => "unexpected identity",
			LocalIdentity => "local identity",
			PingTimeout => "ping timeout",
			Unknown => "unknown",
		};

		f.write_str(msg)
	}
}

/// Queue error
#[derive(Debug, derive_more::Display)]
pub enum Error {
	/// Socket IO error.
	SocketIo(IoError),
	/// Decompression error.
	Decompression(snappy::InvalidInput),
	/// Rlp decoder error.
	Rlp(tetsy_rlp::DecoderError),
	/// Error concerning the network address parsing subsystem.
	#[display(fmt = "Failed to parse network address")]
	AddressParse,
	/// Error concerning the network address resolution subsystem.
	#[display(fmt = "Failed to resolve network address {}", _0)]
	AddressResolve(AddressResolveError),
	/// Authentication failure
	#[display(fmt = "Authentication failure")]
	Auth,
	/// Unrecognised protocol
	#[display(fmt = "Bad protocol")]
	BadProtocol,
	/// Expired message
	#[display(fmt = "Expired message")]
	Expired,
	/// Peer not found
	#[display(fmt = "Peer not found")]
	PeerNotFound,
	/// Peer is disconnected
	#[display(fmt = "Peer disconnected: {}", _0)]
	Disconnect(DisconnectReason),
	/// Invalid node id
	#[display(fmt = "Invalid node id")]
	InvalidNodeId,
	/// Packet size is over the protocol limit
	#[display(fmt = "Packet is too large")]
	OversizedPacket,
	/// Reached system resource limits for this process
	#[display(fmt = "Too many open files in this process. Check your resource limits and restart tetsy")]
	ProcessTooManyFiles,
	/// Reached system wide resource limits
	#[display(fmt = "Too many open files on system. Consider closing some processes/release some file handlers or increas the system-wide resource limits and restart tetsy.")]
	SystemTooManyFiles,
	/// An unknown IO error occurred.
	#[display(fmt = "Unexpected IO error: {}", _0)]
	Io(io::Error),
}

/// Wraps io::Error for Display impl
#[derive(Debug)]
pub struct AddressResolveError(Option<io::Error>);

impl fmt::Display for AddressResolveError {
	fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
		write!(f, "{}", self.0.as_ref().map_or("".to_string(), |e| e.to_string()))
	}
}

impl From<Option<io::Error>> for AddressResolveError {
	fn from(err: Option<io::Error>) -> Self {
		AddressResolveError(err)
	}
}

impl error::Error for Error {
	fn source(&self) -> Option<&(dyn error::Error + 'static)> {
		match self {
			Error::Decompression(e) => Some(e),
			Error::Rlp(e) => Some(e),
			_ => None,
		}
	}
}

impl From<IoError> for Error {
	fn from(err: IoError) -> Self {
		Error::SocketIo(err)
	}
}

impl From<snappy::InvalidInput> for Error {
	fn from(err: snappy::InvalidInput) -> Self {
		Error::Decompression(err)
	}
}

impl From<tetsy_rlp::DecoderError> for Error {
	fn from(err: tetsy_rlp::DecoderError) -> Self {
		Error::Rlp(err)
	}
}

impl From<io::Error> for Error {
	fn from(err: io::Error) -> Self {
		match err.raw_os_error() {
			Some(ENFILE) => Error::ProcessTooManyFiles,
			Some(EMFILE) => Error::SystemTooManyFiles,
			_ => Error::Io(err)
		}
	}
}

impl From<crypto::publickey::Error> for Error {
	fn from(_err: crypto::publickey::Error) -> Self {
		Error::Auth
	}
}

impl From<crypto::error::SymmError> for Error {
	fn from(_err: crypto::error::SymmError) -> Self {
		Error::Auth
	}
}

impl From<net::AddrParseError> for Error {
	fn from(_err: net::AddrParseError) -> Self { Error::AddressParse }
}

#[test]
fn test_errors() {
	assert_eq!(DisconnectReason::ClientQuit, DisconnectReason::from_u8(8));
	let mut r = DisconnectReason::DisconnectRequested;
	for i in 0 .. 20 {
		r = DisconnectReason::from_u8(i);
	}
	assert_eq!(DisconnectReason::Unknown, r);

	match <Error as From<tetsy_rlp::DecoderError>>::from(tetsy_rlp::DecoderError::RlpIsTooBig) {
		Error::Rlp(_) => {},
		_ => panic!("Unexpected error"),
	}

	match <Error as From<crypto::publickey::Error>>::from(crypto::publickey::Error::InvalidMessage) {
		Error::Auth => {},
		_ => panic!("Unexpected error"),
	}
}

#[test]
fn test_io_errors() {
	use libc::{EMFILE, ENFILE};

	assert_matches!(
		<Error as From<io::Error>>::from(
			io::Error::from_raw_os_error(ENFILE)
			),
		Error::ProcessTooManyFiles);

	assert_matches!(
		<Error as From<io::Error>>::from(
			io::Error::from_raw_os_error(EMFILE)
			),
		Error::SystemTooManyFiles);

	assert_matches!(
		<Error as From<io::Error>>::from(
			io::Error::from_raw_os_error(0)
			),
		Error::Io(_));
}
