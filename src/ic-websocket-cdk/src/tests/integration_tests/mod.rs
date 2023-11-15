//! Tests are named with letters to keep them in alphabetical order and ensure they are executed in that order.
//! Make sure you run tests with `--test-threads=1` so that they are executed sequentially.
//!
//! In each module, tests are named using `test_[i]_*` pattern so that they are executed in that order.

#![cfg(test)]

mod utils;

mod a_ws_open;
mod b_ws_message;
mod c_ws_get_messages;
mod d_ws_close;
mod e_ws_send;
mod f_messages_acknowledgement;
mod g_multiple_gateways;
