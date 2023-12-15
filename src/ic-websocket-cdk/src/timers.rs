use ic_cdk_timers::{clear_timer, TimerId};
use ic_cdk_timers::{set_timer, set_timer_interval};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::state::*;
use crate::types::*;
use crate::utils::*;
use crate::{custom_print, CLIENT_KEEP_ALIVE_TIMEOUT_MS, CLIENT_KEEP_ALIVE_TIMEOUT_NS};

thread_local! {
  /// The acknowledgement active timer.
  /* flexible */ pub(crate) static ACK_TIMER: RefCell<Option<TimerId>> = RefCell::new(None);
  /// The keep alive active timer.
  /* flexible */ pub(crate) static KEEP_ALIVE_TIMER: RefCell<Option<TimerId>> = RefCell::new(None);
}

fn put_ack_timer_id(timer_id: TimerId) {
    ACK_TIMER.with(|timer| timer.borrow_mut().replace(timer_id));
}

fn cancel_ack_timer() {
    if let Some(t_id) = ACK_TIMER.with(|timer| timer.borrow_mut().take()) {
        clear_timer(t_id);
    }
}

fn put_keep_alive_timer_id(timer_id: TimerId) {
    KEEP_ALIVE_TIMER.with(|timer| timer.borrow_mut().replace(timer_id));
}

fn cancel_keep_alive_timer() {
    if let Some(t_id) = KEEP_ALIVE_TIMER.with(|timer| timer.borrow_mut().take()) {
        clear_timer(t_id);
    }
}

pub(crate) fn cancel_timers() {
    cancel_ack_timer();
    cancel_keep_alive_timer();
}

/// Start an interval to send an acknowledgement messages to the clients.
///
/// The interval callback is [send_ack_to_clients_timer_callback]. After the callback is executed,
/// a timer is scheduled to check if the registered clients have sent a keep alive message.
pub(crate) fn schedule_send_ack_to_clients() {
    let ack_interval_ms = get_params().send_ack_interval_ms;
    let timer_id = set_timer_interval(Duration::from_millis(ack_interval_ms), move || {
        send_ack_to_clients_timer_callback();

        schedule_check_keep_alive();
    });

    put_ack_timer_id(timer_id);
}

/// Schedules a timer to check if the clients (only those to which an ack message was sent) have sent a keep alive message
/// after receiving an acknowledgement message.
///
/// The timer callback is [check_keep_alive_timer_callback].
fn schedule_check_keep_alive() {
    let timer_id = set_timer(
        Duration::from_millis(CLIENT_KEEP_ALIVE_TIMEOUT_MS),
        move || {
            check_keep_alive_timer_callback();
        },
    );

    put_keep_alive_timer_id(timer_id);
}

/// Sends an acknowledgement message to the client.
/// The message contains the current incoming message sequence number for that client,
/// so that the client knows that all the messages it sent have been received by the canister.
fn send_ack_to_clients_timer_callback() {
    for client_key in REGISTERED_CLIENTS.with(Rc::clone).borrow().keys() {
        // ignore the error, which shouldn't happen since the client is registered and the sequence number is initialized
        match get_expected_incoming_message_from_client_num(client_key) {
            Ok(expected_incoming_sequence_num) => {
                let ack_message = CanisterAckMessageContent {
                    // the expected sequence number is 1 more because it's incremented when a message is received
                    last_incoming_sequence_num: expected_incoming_sequence_num - 1,
                };
                let message = WebsocketServiceMessageContent::AckMessage(ack_message);
                if let Err(e) = send_service_message_to_client(client_key, &message) {
                    // TODO: decide what to do when sending the message fails

                    custom_print!(
                        "[ack-to-clients-timer-cb]: Error sending ack message to client {}: {:?}",
                        client_key,
                        e
                    );
                } else {
                    add_client_to_wait_for_keep_alive(client_key);
                }
            },
            Err(e) => {
                // TODO: decide what to do when getting the expected incoming sequence number fails (shouldn't happen)
                custom_print!(
                  "[ack-to-clients-timer-cb]: Error getting expected incoming sequence number for client {}: {:?}",
                  client_key,
                  e,
              );
            },
        }
    }

    custom_print!("[ack-to-clients-timer-cb]: Sent ack messages to all clients");
}

/// Checks if the clients for which we are waiting for keep alive have sent a keep alive message.
/// If a client has not sent a keep alive message, it is removed from the connected clients.
///
/// Before checking the clients, it removes all the empty expired gateways from the list of registered gateways.
fn check_keep_alive_timer_callback() {
    remove_empty_expired_gateways();

    let client_keys_to_remove: Vec<ClientKey> = CLIENTS_WAITING_FOR_KEEP_ALIVE
        .with(Rc::clone)
        .borrow()
        .iter()
        .filter_map(|client_key| {
            // get the last keep alive timestamp for the client and check if it has exceeded the timeout
            if let Some(client_metadata) =
                REGISTERED_CLIENTS.with(Rc::clone).borrow().get(client_key)
            {
                let last_keep_alive = client_metadata.get_last_keep_alive_timestamp();
                if get_current_time() - last_keep_alive > CLIENT_KEEP_ALIVE_TIMEOUT_NS {
                    Some(client_key.to_owned())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    for client_key in client_keys_to_remove {
        remove_client(&client_key, Some(CloseMessageReason::KeepAliveTimeout));

        custom_print!(
          "[check-keep-alive-timer-cb]: Client {} has not sent a keep alive message in the last {}ms and has been removed",
          client_key,
          CLIENT_KEEP_ALIVE_TIMEOUT_MS
      );
    }

    custom_print!("[check-keep-alive-timer-cb]: Checked keep alive messages for all clients");
}
