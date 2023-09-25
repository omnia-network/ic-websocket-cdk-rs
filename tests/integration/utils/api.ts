// helpers for functions that are called frequently in tests

import { ActorSubclass, Cbor, Certificate, HashTree, HttpAgent, compare, lookup_path, reconstruct } from "@dfinity/agent";
import { Secp256k1KeyIdentity } from "@dfinity/identity-secp256k1";
import { Principal } from "@dfinity/principal";
import { IDL } from "@dfinity/candid";
import { anonymousClient, gateway1Data } from "./actors";
import type { CanisterOutputCertifiedMessages, ClientKey, ClientPrincipal, WebsocketMessage, _SERVICE } from "../../src/declarations/test_canister/test_canister.did";

type GenericResult<T> = {
  Ok: T,
} | {
  Err: string,
};

const resolveResult = <T>(result: GenericResult<T>, throwIfError: boolean) => {
  if (throwIfError && 'Err' in result) {
    throw new Error(result.Err);
  }

  return result;
};

type WsOpenArgs = {
  clientNonce: bigint,
  canisterId: string,
  clientActor: ActorSubclass<_SERVICE>,
};

/**
 * Sends an update call to the canister to the **ws_open** method, using the provided actor.
 * @param args {@link WsOpenArgs}
 * @param throwIfError whether to throw if the result is an error (defaults to `false`)
 * @returns the result of the **ws_open** method
 */
export const wsOpen = async (args: WsOpenArgs, throwIfError = false) => {
  const res = await args.clientActor.ws_open({
    client_nonce: args.clientNonce,
  });

  return resolveResult(res, throwIfError);
};

type WsMessageArgs = {
  message: WebsocketMessage,
  actor: ActorSubclass<_SERVICE>,
};

/**
 * Sends an update call to the canister to the **ws_message** method, using the provided actor.
 * @param args {@link WsMessageArgs}
 * @param throwIfError whether to throw if the result is an error (defaults to `false`)
 * @returns the result of the **ws_message** method
 */
export const wsMessage = async (args: WsMessageArgs, throwIfError = false) => {
  const res = await args.actor.ws_message({
    msg: args.message,
  });

  return resolveResult(res, throwIfError);
};

type WsCloseArgs = {
  clientKey: ClientKey,
  gatewayActor: ActorSubclass<_SERVICE>,
};

/**
 * Sends an update call to the canister to the **ws_close** method, using the provided gateway actor.
 * @param args {@link WsCloseArgs}
 * @param throwIfError whether to throw if the result is an error (defaults to `false`)
 * @returns the result of the **ws_close** method
 */
export const wsClose = async (args: WsCloseArgs, throwIfError = false) => {
  const res = await args.gatewayActor.ws_close({
    client_key: args.clientKey,
  });

  return resolveResult(res, throwIfError);
};

type WsGetMessagesArgs = {
  fromNonce: number,
  gatewayActor: ActorSubclass<_SERVICE>,
};

/**
 * Sends a query call to the canister to the **ws_get_messages** method, using the provided gateway actor.
 * @param args {@link WsGetMessagesArgs}
 */
export const wsGetMessages = async (args: WsGetMessagesArgs): Promise<CanisterOutputCertifiedMessages> => {
  const res = await args.gatewayActor.ws_get_messages({
    nonce: BigInt(args.fromNonce),
  });

  const messages = resolveResult(res, true);

  return (messages as { Ok: CanisterOutputCertifiedMessages }).Ok;
};

export const wsWipe = async () => {
  await anonymousClient.ws_wipe();
};

type ReinitializeArgs = {
  sendAckIntervalMs: number,
  keepAliveDelayMs: number,
};

/**
 * Used to reinitialize the canister with the provided intervals.
 * @param args {@link ReinitializeArgs}
 */
export const reinitialize = async (args: ReinitializeArgs) => {
  const gatewayPrincipal = (await gateway1Data.identity).getPrincipal().toText();
  await anonymousClient.reinitialize(gatewayPrincipal, BigInt(args.sendAckIntervalMs), BigInt(args.keepAliveDelayMs));
};

type WsSendArgs = {
  clientPrincipal: ClientPrincipal,
  actor: ActorSubclass<_SERVICE>,
  message: {
    text: string,
  },
};

export const wsSend = async (args: WsSendArgs, throwIfError = false) => {
  const msgBytes = IDL.encode([IDL.Record({ 'text': IDL.Text })], [args.message]);
  const res = await args.actor.ws_send(args.clientPrincipal, new Uint8Array(msgBytes));

  return resolveResult(res, throwIfError);
};

export const getCertifiedMessageKey = async (gatewayIdentity: Promise<Secp256k1KeyIdentity>, nonce: number) => {
  const gatewayPrincipal = (await gatewayIdentity).getPrincipal().toText();
  return `${gatewayPrincipal}_${String(nonce).padStart(20, '0')}`;
};

export const isValidCertificate = async (canisterId: string, certificate: Uint8Array, tree: Uint8Array, agent: HttpAgent) => {
  const canisterPrincipal = Principal.fromText(canisterId);
  let cert: Certificate;

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
