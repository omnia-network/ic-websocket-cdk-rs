import { Cbor } from "@dfinity/agent";
import { CanisterOutputMessage, ClientKey, WebsocketMessage } from "../../src/declarations/test_canister/test_canister.did";
import { getWebsocketMessageFromCanisterMessage } from "./idl";

export const filterServiceMessagesFromCanisterMessages = (messages: CanisterOutputMessage[]): CanisterOutputMessage[] => {
  return messages.filter((msg) => {
    const websocketMessage = getWebsocketMessageFromCanisterMessage(msg);
    return websocketMessage.is_service_message;
  });
};

export const createWebsocketMessage = (
  clientKey: ClientKey,
  sequenceNumber: number,
  isServiceMessage = false,
  content?: ArrayBuffer | Uint8Array
): WebsocketMessage => {
  const websocketMessage: WebsocketMessage = {
    client_key: clientKey,
    sequence_num: BigInt(sequenceNumber),
    timestamp: BigInt(Date.now()) * BigInt(1_000_000), // in nanoseconds
    content: new Uint8Array(content || []),
    is_service_message: isServiceMessage,
  };

  return websocketMessage;
};

export const decodeWebsocketMessage = (bytes: Uint8Array): WebsocketMessage => {
  return Cbor.decode(bytes);
};
