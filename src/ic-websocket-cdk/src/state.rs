use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    time::Duration,
};

use candid::{encode_one, Principal};
#[allow(unused_imports)]
use ic_cdk::api::{data_certificate, set_certified_data};
use ic_certified_map::{labeled, labeled_hash, AsHashTree, Hash as ICHash, RbTree};
use serde::Serialize;
use serde_cbor::Serializer;
use sha2::{Digest, Sha256};

use crate::{
    errors::WsError, types::*, utils::get_current_time, INITIAL_CANISTER_SEQUENCE_NUM,
    INITIAL_CLIENT_SEQUENCE_NUM, LABEL_WEBSOCKET, MESSAGES_TO_DELETE_COUNT,
};

thread_local! {
  /// Maps the client's key to the client metadata
  /* flexible */ pub(crate) static REGISTERED_CLIENTS: Rc<RefCell<HashMap<ClientKey, RegisteredClient>>> = Rc::new(RefCell::new(HashMap::new()));
  /// Maps the client's principal to the current client key
  /* flexible */ pub(crate) static CURRENT_CLIENT_KEY_MAP: RefCell<HashMap<ClientPrincipal, ClientKey>> = RefCell::new(HashMap::new());
  /// Keeps track of all the clients for which we're waiting for a keep alive message
  /* flexible */ pub(crate) static CLIENTS_WAITING_FOR_KEEP_ALIVE: Rc<RefCell<HashSet<ClientKey>>> = Rc::new(RefCell::new(HashSet::new()));
  /// Maps the client's key to the sequence number to use for the next outgoing message (to that client).
  /* flexible */ pub(crate) static OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP: RefCell<HashMap<ClientKey, u64>> = RefCell::new(HashMap::new());
  /// Maps the client's key to the expected sequence number of the next incoming message (from that client).
  /* flexible */ pub(crate) static INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP: RefCell<HashMap<ClientKey, u64>> = RefCell::new(HashMap::new());
  /// Keeps track of the Merkle tree used for certified queries
  /* flexible */ pub(crate) static CERT_TREE: RefCell<RbTree<String, ICHash>> = RefCell::new(RbTree::new());
  /// Keeps track of the principals of the WS Gateways that poll the canister
  /* flexible */ pub(crate) static REGISTERED_GATEWAYS: RefCell<HashMap<GatewayPrincipal, RegisteredGateway>> = RefCell::new(HashMap::new());
  /// Keeps track of the gateways that must be removed from the list of registered gateways in the next ack interval
  /* flexible */ pub(crate) static GATEWAYS_TO_REMOVE: RefCell<HashMap<GatewayPrincipal, TimestampNs>> = RefCell::new(HashMap::new());
  /// The parameters passed in the CDK initialization
  /* flexible */ pub(crate) static PARAMS: RefCell<WsInitParams> = RefCell::new(WsInitParams::default());
}

/// Resets all RefCells to their initial state.
pub(crate) fn reset_internal_state() {
    let client_keys_to_remove: Vec<ClientKey> = REGISTERED_CLIENTS.with(|state| {
        let map = state.borrow();
        map.keys().cloned().collect()
    });

    // for each client, call the on_close handler before clearing the map
    for client_key in client_keys_to_remove {
        remove_client(&client_key, None);
    }

    // make sure all the maps are cleared
    CURRENT_CLIENT_KEY_MAP.with(|map| {
        map.borrow_mut().clear();
    });
    CLIENTS_WAITING_FOR_KEEP_ALIVE.with(|set| {
        set.borrow_mut().clear();
    });
    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut().clear();
    });
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut().clear();
    });
    CERT_TREE.with(|t| {
        t.replace(RbTree::new());
    });
    REGISTERED_GATEWAYS.with(|map| {
        map.borrow_mut().clear();
    });
    GATEWAYS_TO_REMOVE.with(|map| {
        map.borrow_mut().clear();
    });
}

/// Increments the clients connected count for the given gateway.
/// If the gateway is not registered, a new entry is created with a clients connected count of 1.
pub(crate) fn increment_gateway_clients_count(gateway_principal: GatewayPrincipal) {
    GATEWAYS_TO_REMOVE.with(|state| {
        state.borrow_mut().remove(&gateway_principal);
    });

    REGISTERED_GATEWAYS.with(|map| {
        map.borrow_mut()
            .entry(gateway_principal)
            .or_insert_with(RegisteredGateway::new)
            .increment_clients_count();
    });
}

/// Decrements the clients connected count for the given gateway, if it exists.
///
/// If the gateway has no more clients connected, it is added to the [GATEWAYS_TO_REMOVE] map,
/// in order to remove it in the next keep alive check.
pub(crate) fn decrement_gateway_clients_count(gateway_principal: &GatewayPrincipal) {
    let is_empty = REGISTERED_GATEWAYS.with(|map| {
        map.borrow_mut()
            .get_mut(gateway_principal)
            .is_some_and(|g| {
                let clients_count = g.decrement_clients_count();
                clients_count == 0
            })
    });

    if is_empty {
        GATEWAYS_TO_REMOVE.with(|state| {
            state
                .borrow_mut()
                .insert(gateway_principal.clone(), get_current_time());
        });
    }
}

/// Removes the gateways that were added to the [GATEWAYS_TO_REMOVE] map
/// more than the ack interval ms time ago from the list of registered gateways
pub(crate) fn remove_empty_expired_gateways() {
    let ack_interval_ms = get_params().send_ack_interval_ms;
    let time = get_current_time();

    let mut gateway_principals_to_remove: Vec<GatewayPrincipal> = vec![];

    GATEWAYS_TO_REMOVE.with(|state| {
        state.borrow_mut().retain(|gp, added_at| {
            if Duration::from_nanos(time - *added_at) > Duration::from_millis(ack_interval_ms) {
                gateway_principals_to_remove.push(gp.clone());
                false
            } else {
                true
            }
        })
    });

    for gateway_principal in &gateway_principals_to_remove {
        if let Some(messages_keys_to_delete) = REGISTERED_GATEWAYS.with(|map| {
            map.borrow_mut()
                .remove(gateway_principal)
                .map(|g| g.messages_queue.iter().map(|m| m.key.clone()).collect())
        }) {
            delete_keys_from_cert_tree(messages_keys_to_delete);
        }
    }
}

pub(crate) fn get_registered_gateway(
    gateway_principal: &GatewayPrincipal,
) -> Result<RegisteredGateway, String> {
    REGISTERED_GATEWAYS.with(|map| {
        map.borrow()
            .get(gateway_principal)
            .ok_or_else(|| WsError::GatewayNotRegistered { gateway_principal }.to_string())
            .cloned()
    })
}

pub(crate) fn check_is_gateway_registered(
    gateway_principal: &GatewayPrincipal,
) -> Result<(), String> {
    get_registered_gateway(gateway_principal).map(|_| ())
}

pub(crate) fn is_registered_gateway(principal: &Principal) -> bool {
    get_registered_gateway(principal).is_ok()
}

pub(crate) fn get_outgoing_message_nonce(
    gateway_principal: &GatewayPrincipal,
) -> Result<u64, String> {
    get_registered_gateway(gateway_principal).map(|g| g.outgoing_message_nonce)
}

pub(crate) fn increment_outgoing_message_nonce(
    gateway_principal: &GatewayPrincipal,
) -> Result<(), String> {
    REGISTERED_GATEWAYS.with(|map| {
        map.borrow_mut()
            .get_mut(gateway_principal)
            .and_then(|g| {
                g.increment_nonce();
                Some(())
            })
            .ok_or_else(|| WsError::GatewayNotRegistered { gateway_principal }.to_string())
    })
}

pub(crate) fn insert_client(client_key: ClientKey, new_client: RegisteredClient) {
    CURRENT_CLIENT_KEY_MAP.with(|map| {
        map.borrow_mut()
            .insert(client_key.client_principal, client_key.clone());
    });
    REGISTERED_CLIENTS.with(|map| {
        map.borrow_mut().insert(client_key, new_client);
    });
}

pub(crate) fn get_registered_client(client_key: &ClientKey) -> Result<RegisteredClient, String> {
    REGISTERED_CLIENTS.with(|map| {
        map.borrow()
            .get(client_key)
            .ok_or_else(|| WsError::ClientKeyNotConnected { client_key }.to_string())
            .cloned()
    })
}

pub(crate) fn get_client_key_from_principal(
    client_principal: &ClientPrincipal,
) -> Result<ClientKey, String> {
    CURRENT_CLIENT_KEY_MAP.with(|map| {
        map.borrow()
            .get(client_principal)
            .cloned()
            .ok_or_else(|| WsError::ClientPrincipalNotConnected { client_principal }.to_string())
    })
}

pub(crate) fn check_registered_client_exists(client_key: &ClientKey) -> Result<(), String> {
    get_registered_client(client_key).map(|_| ())
}

pub(crate) fn check_client_registered_to_gateway(
    client_key: &ClientKey,
    gateway_principal: &GatewayPrincipal,
) -> Result<(), String> {
    get_registered_client(client_key).and_then(|registered_client| {
        registered_client
            .gateway_principal
            .eq(gateway_principal)
            .then_some(())
            .ok_or_else(|| {
                WsError::ClientNotRegisteredToGateway {
                    client_key,
                    gateway_principal,
                }
                .to_string()
            })
    })
}

pub(crate) fn add_client_to_wait_for_keep_alive(client_key: &ClientKey) {
    CLIENTS_WAITING_FOR_KEEP_ALIVE.with(|clients| {
        clients.borrow_mut().insert(client_key.clone());
    });
}

pub(crate) fn init_outgoing_message_to_client_num(client_key: ClientKey) {
    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut()
            .insert(client_key, INITIAL_CANISTER_SEQUENCE_NUM);
    });
}

pub(crate) fn get_outgoing_message_to_client_num(client_key: &ClientKey) -> Result<u64, String> {
    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        map.borrow().get(client_key).cloned().ok_or_else(|| {
            WsError::OutgoingMessageToClientNumNotInitialized { client_key }.to_string()
        })
    })
}

pub(crate) fn increment_outgoing_message_to_client_num(
    client_key: &ClientKey,
) -> Result<(), String> {
    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut()
            .get_mut(client_key)
            .ok_or_else(|| {
                WsError::OutgoingMessageToClientNumNotInitialized { client_key }.to_string()
            })
            .map(|n| *n += 1)
    })
}

pub(crate) fn init_expected_incoming_message_from_client_num(client_key: ClientKey) {
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut()
            .insert(client_key, INITIAL_CLIENT_SEQUENCE_NUM);
    });
}

pub(crate) fn get_expected_incoming_message_from_client_num(
    client_key: &ClientKey,
) -> Result<u64, String> {
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        map.borrow().get(client_key).cloned().ok_or(
            WsError::ExpectedIncomingMessageToClientNumNotInitialized { client_key }.to_string(),
        )
    })
}

pub(crate) fn increment_expected_incoming_message_from_client_num(
    client_key: &ClientKey,
) -> Result<(), String> {
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut()
            .get_mut(client_key)
            .ok_or(
                WsError::ExpectedIncomingMessageToClientNumNotInitialized { client_key }
                    .to_string(),
            )
            .map(|n| *n += 1)
    })
}

pub(crate) fn add_client(client_key: ClientKey, new_client: RegisteredClient) {
    // insert the client in the map
    insert_client(client_key.clone(), new_client.clone());
    // initialize incoming client's message sequence number to 1
    init_expected_incoming_message_from_client_num(client_key.clone());
    // initialize outgoing message sequence number to 0
    init_outgoing_message_to_client_num(client_key);

    increment_gateway_clients_count(new_client.gateway_principal);
}

/// Removes a client from the internal state and call the on_close callback,
/// if the client was registered in the state.
///
/// If a `close_reason` is provided, it also sends a close message to the client,
/// so that the client can close the WS connection with the gateway.
pub(crate) fn remove_client(client_key: &ClientKey, close_reason: Option<CloseMessageReason>) {
    if let Some(close_reason) = close_reason.clone() {
        // ignore the error
        let _ = send_service_message_to_client(
            client_key,
            &WebsocketServiceMessageContent::CloseMessage(CanisterCloseMessageContent {
                reason: close_reason,
            }),
        );
    }

    CLIENTS_WAITING_FOR_KEEP_ALIVE.with(|set| {
        set.borrow_mut().remove(client_key);
    });
    CURRENT_CLIENT_KEY_MAP.with(|map| {
        map.borrow_mut().remove(&client_key.client_principal);
    });
    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut().remove(client_key);
    });
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut().remove(client_key);
    });

    if let Some(registered_client) =
        REGISTERED_CLIENTS.with(|map| map.borrow_mut().remove(client_key))
    {
        decrement_gateway_clients_count(&registered_client.gateway_principal);

        let handlers = get_handlers_from_params();
        handlers.call_on_close(OnCloseCallbackArgs {
            client_principal: client_key.client_principal,
        });
    };
}

pub(crate) fn format_message_for_gateway_key(
    gateway_principal: &GatewayPrincipal,
    nonce: u64,
) -> String {
    gateway_principal.to_string() + "_" + &format!("{:0>20}", nonce.to_string())
}

pub(crate) fn get_messages_for_gateway_range(
    gateway_principal: &GatewayPrincipal,
    nonce: u64,
) -> MessagesForGatewayRange {
    let max_number_of_returned_messages = get_params().max_number_of_returned_messages;

    let messages_queue = get_registered_gateway(gateway_principal)
        .unwrap() // the value exists because we just checked that the gateway is registered
        .messages_queue;

    let queue_len = messages_queue.len();

    // smallest key used to determine the first message from the queue which has to be returned to the WS Gateway
    let smallest_key = format_message_for_gateway_key(gateway_principal, nonce);
    // partition the queue at the message which has the key with the nonce specified as argument to get_cert_messages
    let start_index = messages_queue.partition_point(|x| x.key < smallest_key);
    // message at index corresponding to end index is excluded
    let (end_index, is_end_of_queue) = if queue_len - start_index > max_number_of_returned_messages
    {
        (start_index + max_number_of_returned_messages, false)
    } else {
        (queue_len, true)
    };

    MessagesForGatewayRange {
        start_index,
        end_index,
        is_end_of_queue,
    }
}

pub(crate) fn get_messages_for_gateway(
    gateway_principal: &GatewayPrincipal,
    start_index: usize,
    end_index: usize,
) -> Vec<CanisterOutputMessage> {
    let messages_queue = get_registered_gateway(gateway_principal)
        .unwrap() // the value exists because we just checked that the gateway is registered
        .messages_queue;
    let mut messages: Vec<CanisterOutputMessage> = Vec::with_capacity(end_index - start_index);
    for index in start_index..end_index {
        messages.push(
            messages_queue
                .get(index)
                .unwrap() // the value exists because this function is called only after partitioning the queue
                .clone(),
        );
    }
    messages
}

/// Gets the messages in [MESSAGES_FOR_GATEWAYS] starting from the one with the specified nonce
pub(crate) fn get_cert_messages(
    gateway_principal: &GatewayPrincipal,
    nonce: u64,
) -> CanisterWsGetMessagesResult {
    let MessagesForGatewayRange {
        start_index,
        end_index,
        is_end_of_queue,
    } = get_messages_for_gateway_range(gateway_principal, nonce);
    let messages = get_messages_for_gateway(gateway_principal, start_index, end_index);

    if messages.is_empty() {
        return get_cert_messages_empty();
    }

    let first_key = messages.first().unwrap().key.clone();
    let last_key = messages.last().unwrap().key.clone();
    let (cert, tree) = get_cert_for_range(&first_key, &last_key);

    Ok(CanisterOutputCertifiedMessages {
        messages,
        cert,
        tree,
        is_end_of_queue,
    })
}

pub(crate) fn get_cert_messages_empty() -> CanisterWsGetMessagesResult {
    Ok(CanisterOutputCertifiedMessages::empty())
}

pub(crate) fn put_cert_for_message(key: String, value: &Vec<u8>) {
    #[allow(unused_variables)]
    let root_hash = CERT_TREE.with(|tree| {
        let mut tree = tree.borrow_mut();
        tree.insert(key.clone(), Sha256::digest(value).into());
        labeled_hash(LABEL_WEBSOCKET, &tree.root_hash())
    });

    #[cfg(not(test))]
    // executing this in unit tests fails because it's an IC-specific API
    set_certified_data(&root_hash);
}

/// Adds the message to the gateway queue.
pub(crate) fn push_message_in_gateway_queue(
    gateway_principal: &GatewayPrincipal,
    message: CanisterOutputMessage,
    message_timestamp: u64,
) -> Result<(), String> {
    REGISTERED_GATEWAYS.with(|map| {
        // messages in the queue are inserted with contiguous and increasing nonces
        // (from beginning to end of the queue) as `send` is called sequentially, the nonce
        // is incremented by one in each call, and the message is pushed at the end of the queue
        map.borrow_mut()
            .get_mut(gateway_principal)
            .ok_or_else(|| WsError::GatewayNotRegistered { gateway_principal }.to_string())
            .and_then(|g| {
                g.add_message_to_queue(message, message_timestamp);
                Ok(())
            })
    })
}

/// Deletes the an amount of [MESSAGES_TO_DELETE] messages from the queue
/// that are older than the ack interval.
pub(crate) fn delete_old_messages_for_gateway(
    gateway_principal: &GatewayPrincipal,
) -> Result<(), String> {
    let ack_interval_ms = get_params().send_ack_interval_ms;

    let deleted_messages_keys = REGISTERED_GATEWAYS.with(|map| {
        map.borrow_mut()
            .get_mut(gateway_principal)
            .ok_or_else(|| WsError::GatewayNotRegistered { gateway_principal }.to_string())
            .and_then(|g| Ok(g.delete_old_messages(MESSAGES_TO_DELETE_COUNT, ack_interval_ms)))
    })?;

    delete_keys_from_cert_tree(deleted_messages_keys);

    Ok(())
}

pub(crate) fn delete_keys_from_cert_tree(keys: Vec<String>) {
    #[allow(unused_variables)]
    let root_hash = CERT_TREE.with(|tree| {
        let mut tree = tree.borrow_mut();
        for key in keys {
            tree.delete(key.as_ref());
        }
        labeled_hash(LABEL_WEBSOCKET, &tree.root_hash())
    });

    // certify data with the new root hash
    #[cfg(not(test))]
    // executing this in unit tests fails because it's an IC-specific API
    set_certified_data(&root_hash);
}

fn get_cert_for_range(first: &String, last: &String) -> (Vec<u8>, Vec<u8>) {
    CERT_TREE.with(|tree| {
        let tree = tree.borrow();
        let witness = tree.value_range(first.as_ref(), last.as_ref());
        let tree = labeled(LABEL_WEBSOCKET, witness);

        let mut data = vec![];
        let mut serializer = Serializer::new(&mut data);
        serializer.self_describe().unwrap();
        tree.serialize(&mut serializer).unwrap();
        (data_certificate().unwrap(), data)
    })
}

pub(crate) fn set_params(params: WsInitParams) {
    PARAMS.with(|state| *state.borrow_mut() = params);
}

pub(crate) fn get_params() -> WsInitParams {
    PARAMS.with(|state| state.borrow().clone())
}

pub(crate) fn get_handlers_from_params() -> WsHandlers {
    get_params().get_handlers()
}

fn handle_keep_alive_client_message(
    client_key: &ClientKey,
    _keep_alive_message: ClientKeepAliveMessageContent,
) {
    // update the last keep alive timestamp for the client
    if let Some(client_metadata) = REGISTERED_CLIENTS
        .with(Rc::clone)
        .borrow_mut()
        .get_mut(client_key)
    {
        client_metadata.update_last_keep_alive_timestamp();
    }
}

pub(crate) fn handle_received_service_message(
    client_key: &ClientKey,
    content: &[u8],
) -> CanisterWsMessageResult {
    let decoded = WebsocketServiceMessageContent::from_candid_bytes(content)?;
    match decoded {
        WebsocketServiceMessageContent::OpenMessage(_)
        | WebsocketServiceMessageContent::AckMessage(_)
        | WebsocketServiceMessageContent::CloseMessage(_) => {
            WsError::InvalidServiceMessage.to_string_result()
        },
        WebsocketServiceMessageContent::KeepAliveMessage(keep_alive_message) => {
            handle_keep_alive_client_message(client_key, keep_alive_message);
            Ok(())
        },
    }
}

pub(crate) fn send_service_message_to_client(
    client_key: &ClientKey,
    message: &WebsocketServiceMessageContent,
) -> Result<(), String> {
    let message_bytes = encode_one(&message).unwrap();
    _ws_send(client_key, message_bytes, true)
}

/// Internal function used to put the messages in the outgoing messages queue and certify them.
pub(crate) fn _ws_send(
    client_key: &ClientKey,
    msg_bytes: Vec<u8>,
    is_service_message: bool,
) -> CanisterSendResult {
    // get the registered client if it exists
    let registered_client = get_registered_client(client_key)?;

    // the nonce in key is used by the WS Gateway to determine the message to start in the polling iteration
    // the key is also passed to the client in order to validate the body of the certified message
    let outgoing_message_nonce = get_outgoing_message_nonce(&registered_client.gateway_principal)?;
    let message_key = format_message_for_gateway_key(
        &registered_client.gateway_principal,
        outgoing_message_nonce,
    );

    // increment the nonce for the next message
    increment_outgoing_message_nonce(&registered_client.gateway_principal)?;

    // increment the sequence number for the next message to the client
    increment_outgoing_message_to_client_num(client_key)?;

    let message_timestamp = get_current_time();

    let websocket_message = WebsocketMessage {
        client_key: client_key.clone(),
        sequence_num: get_outgoing_message_to_client_num(client_key)?,
        timestamp: message_timestamp,
        is_service_message,
        content: msg_bytes,
    };

    // CBOR serialize message of type WebsocketMessage
    let message_content = websocket_message.cbor_serialize()?;

    // delete old messages from the gateway queue
    delete_old_messages_for_gateway(&registered_client.gateway_principal)?;

    // certify data
    put_cert_for_message(message_key.clone(), &message_content);

    // push message in gateway queue
    push_message_in_gateway_queue(
        &registered_client.gateway_principal,
        CanisterOutputMessage {
            client_key: client_key.clone(),
            content: message_content,
            key: message_key,
        },
        message_timestamp,
    )
}
