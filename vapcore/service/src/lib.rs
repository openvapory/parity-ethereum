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

extern crate ansi_term;
extern crate common_types;
extern crate client_traits;
extern crate vapcore;
extern crate vapcore_blockchain as blockchain;
extern crate vapcore_io as io;
extern crate private_tx;
extern crate vapcore_sync as sync;
extern crate vapory_types;
extern crate tetsy_kvdb;
extern crate vapcore_spec;
extern crate vapcore_snapshot;

#[macro_use]
extern crate log;
#[macro_use]
extern crate trace_time;

#[cfg(test)]
extern crate vapcore_db;
#[cfg(test)]
extern crate tempdir;

mod service;

#[cfg(test)]
extern crate tetsy_kvdb_rocksdb;

pub use service::{ClientService, PrivateTxService};
