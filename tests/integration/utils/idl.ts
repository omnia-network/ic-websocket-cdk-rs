import { IDL } from "@dfinity/candid";
import { Actor, ActorSubclass, Cbor } from "@dfinity/agent";
import type { CanisterOutputMessage, ClientKey, WebsocketMessage, _SERVICE } from "../../src/declarations/test_canister/test_canister.did";

export const ClientPrincipalIdl = IDL.Principal;
export const ClientKeyIdl = IDL.Record({
  'client_principal': ClientPrincipalIdl,
  'client_nonce': IDL.Nat64,
});

export type CanisterOpenMessageContent = {
  'client_key': ClientKey,
};
export type CanisterAckMessageContent = {
  'last_incoming_sequence_num': bigint,
};
export type ClientKeepAliveMessageContent = {
  'last_incoming_sequence_num': bigint,
};
export type WebsocketServiceMessageContent = {
  OpenMessage: CanisterOpenMessageContent,
} | {
  AckMessage: CanisterAckMessageContent,
} | {
  KeepAliveMessage: ClientKeepAliveMessageContent,
};

export const CanisterOpenMessageContentIdl = IDL.Record({
  'client_key': ClientKeyIdl,
});
export const CanisterAckMessageContentIdl = IDL.Record({
  'last_incoming_sequence_num': IDL.Nat64,
});
export const ClientKeepAliveMessageContentIdl = IDL.Record({
  'last_incoming_sequence_num': IDL.Nat64,
});
export const WebsocketServiceMessageContentIdl = IDL.Variant({
  'OpenMessage': CanisterOpenMessageContentIdl,
  'AckMessage': CanisterAckMessageContentIdl,
  'KeepAliveMessage': ClientKeepAliveMessageContentIdl,
});

export const decodeWebsocketServiceMessageContent = (bytes: Uint8Array): WebsocketServiceMessageContent => {
  const decoded = IDL.decode([WebsocketServiceMessageContentIdl], bytes);
  if (decoded.length !== 1) {
    throw new Error("Invalid CanisterServiceMessage");
  }
  return decoded[0] as unknown as WebsocketServiceMessageContent;
};

export const encodeWebsocketServiceMessageContent = (msg: WebsocketServiceMessageContent): Uint8Array => {
  return new Uint8Array(IDL.encode([WebsocketServiceMessageContentIdl], [msg]));
};

export const isClientKeyEq = (a: ClientKey, b: ClientKey): boolean => {
  return a.client_principal.compareTo(b.client_principal) === "eq" && a.client_nonce === b.client_nonce;
}

export const getServiceMessageContentFromCanisterMessage = (msg: CanisterOutputMessage): WebsocketServiceMessageContent => {
  const content = getWebsocketMessageFromCanisterMessage(msg).content;
  return decodeWebsocketServiceMessageContent(content as Uint8Array);
}

export const getWebsocketMessageFromCanisterMessage = (msg: CanisterOutputMessage): WebsocketMessage => {
  const websocketMessage: WebsocketMessage = Cbor.decode(msg.content as Uint8Array);
  return websocketMessage;
}

/**
 * Extracts the message type from the canister service definition.
 * 
 * @throws {Error} if the canister does not implement the ws_message method
 * @throws {Error} if the application message type is not optional
 * 
 * COPIED from IC WebSocket JS SDK.
 */
export const extractApplicationMessageIdlFromActor = <T>(actor: ActorSubclass<_SERVICE>): IDL.Type<T> => {
  const wsMessageMethod = Actor.interfaceOf(actor)._fields.find((f) => f[0] === "ws_message");

  if (!wsMessageMethod) {
    throw new Error("Canister does not implement ws_message method");
  }

  if (wsMessageMethod[1].argTypes.length !== 2) {
    throw new Error("ws_message method must have 2 arguments");
  }

  const applicationMessageArg = wsMessageMethod[1].argTypes[1] as IDL.OptClass<T>;
  if (!(applicationMessageArg instanceof IDL.OptClass)) {
    throw new Error("Application message type must be optional in the ws_message arguments");
  }

  return applicationMessageArg["_type"]; // extract the underlying option type
};
