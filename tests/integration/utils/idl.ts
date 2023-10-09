import { IDL } from "@dfinity/candid";
import { CanisterOutputMessage, ClientKey, WebsocketMessage } from "../../src/declarations/test_canister/test_canister.did";
import { Cbor } from "@dfinity/agent";

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

export const getServiceMessageFromCanisterMessage = (msg: CanisterOutputMessage): WebsocketServiceMessageContent => {
  const content = getWebsocketMessageFromCanisterMessage(msg).content;
  return decodeWebsocketServiceMessageContent(content as Uint8Array);
}

export const getWebsocketMessageFromCanisterMessage = (msg: CanisterOutputMessage): WebsocketMessage => {
  const websocketMessage: WebsocketMessage = Cbor.decode(msg.content as Uint8Array);
  return websocketMessage;
}
