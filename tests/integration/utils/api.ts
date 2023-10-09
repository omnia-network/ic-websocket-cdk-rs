// helpers for functions that are called frequently in tests

import { ActorSubclass } from "@dfinity/agent";
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
  messages: Array<{
    text: string,
  }>,
};

export const wsSend = async (args: WsSendArgs, throwIfError = false) => {
  const serializedMessages = args.messages.map((msg) => {
    return new Uint8Array(IDL.encode([IDL.Record({ 'text': IDL.Text })], [msg]));
  });
  const res = await args.actor.ws_send(args.clientPrincipal, serializedMessages);

  return resolveResult(res, throwIfError);
};
