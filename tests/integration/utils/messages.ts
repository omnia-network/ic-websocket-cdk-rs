import { Cbor, Certificate, HashTree, HttpAgent, compare, lookup_path, reconstruct } from "@dfinity/agent";
import { getWebsocketMessageFromCanisterMessage } from "./idl";
import { Principal } from "@dfinity/principal";
import { Secp256k1KeyIdentity } from "@dfinity/identity-secp256k1";
import type { CanisterOutputMessage, ClientKey, WebsocketMessage } from "../../src/declarations/test_canister/test_canister.did";

export const filterServiceMessagesFromCanisterMessages = (messages: CanisterOutputMessage[]): CanisterOutputMessage[] => {
  return messages.filter((msg) => {
    const websocketMessage = getWebsocketMessageFromCanisterMessage(msg);
    return websocketMessage.is_service_message;
  });
};

export const createWebsocketMessage = (
  clientKey: ClientKey,
  sequenceNumber: number,
  content?: ArrayBuffer | Uint8Array,
  isServiceMessage = false,
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
  const decoded: any = Cbor.decode(bytes);

  // normalize the decoded message
  return {
    client_key: {
      client_principal: Principal.fromUint8Array(decoded.client_key.client_principal),
      client_nonce: BigInt(decoded.client_key.client_nonce),
    },
    sequence_num: BigInt(decoded.sequence_num), // not clear why cbor deserializes bigint as number
    timestamp: BigInt(decoded.timestamp),
    content: decoded.content,
    is_service_message: decoded.is_service_message,
  }
};

export const getPollingNonceFromMessage = (message: CanisterOutputMessage): number => {
  const nonceStr = message.key.split("_")[1];
  return parseInt(nonceStr);
};

export const getNextPollingNonceFromMessages = (messages: CanisterOutputMessage[]): number => {
  return getPollingNonceFromMessage(messages[messages.length - 1]) + 1;
};

export const getCertifiedMessageKey = async (gatewayIdentity: Promise<Secp256k1KeyIdentity>, nonce: number) => {
  const gatewayPrincipal = (await gatewayIdentity).getPrincipal().toText();
  return `${gatewayPrincipal}_${String(nonce).padStart(20, '0')}`;
};

export const isValidCertificate = async (canisterId: string, certificate: Uint8Array, tree: Uint8Array, agent: HttpAgent) => {
  const canisterPrincipal = Principal.fromText(canisterId);
  let cert: Certificate;

  if (!agent["_rootKeyFetched"]) {
    await agent.fetchRootKey();
  }

  try {
    cert = await Certificate.create({
      certificate,
      canisterId: canisterPrincipal,
      rootKey: agent.rootKey!
    });
  } catch (error) {
    console.error("Error creating certificate:", error);
    return false;
  }

  const hashTree = Cbor.decode<HashTree>(tree);
  const reconstructed = await reconstruct(hashTree);
  const witness = cert.lookup([
    "canister",
    canisterPrincipal.toUint8Array(),
    "certified_data"
  ]);

  if (!witness) {
    throw new Error(
      "Could not find certified data for this canister in the certificate."
    );
  }

  // First validate that the Tree is as good as the certification.
  return compare(witness, reconstructed) === 0;
};

export const isMessageBodyValid = async (path: string, body: Uint8Array | ArrayBuffer, tree: Uint8Array) => {
  const hashTree = Cbor.decode<HashTree>(tree);
  const sha = await crypto.subtle.digest("SHA-256", body);
  let treeSha = lookup_path(["websocket", path], hashTree);

  if (!treeSha) {
    // Allow fallback to index path.
    treeSha = lookup_path(["websocket"], hashTree);
  }

  return !!treeSha && (compare(sha, treeSha) === 0);
};
