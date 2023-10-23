import { IDL } from "@dfinity/candid";
import {
  anonymousClient,
  canisterId,
  client1,
  client1Data,
  client2,
  client2Data,
  commonAgent,
  gateway1,
  gateway2,
} from "./utils/actors";
import {
  initializeCdk,
  wsClose,
  wsGetMessages,
  wsMessage,
  wsOpen,
  wsSend,
  wsWipe,
} from "./utils/api";
import type {
  CanisterOutputCertifiedMessages,
  CanisterWsCloseResult,
  CanisterWsGetMessagesResult,
  CanisterWsMessageResult,
  CanisterWsOpenResult,
  CanisterWsSendResult,
  ClientKey,
  WebsocketMessage,
} from "../src/declarations/test_canister/test_canister.did";
import { generateClientKey, getRandomClientNonce } from "./utils/random";
import {
  CanisterOpenMessageContent,
  WebsocketServiceMessageContent,
  decodeWebsocketServiceMessageContent,
  encodeWebsocketServiceMessageContent,
  getServiceMessageContentFromCanisterMessage,
  getWebsocketMessageFromCanisterMessage,
  isClientKeyEq,
} from "./utils/idl";
import {
  isMessageBodyValid,
  isValidCertificate,
  createWebsocketMessage,
  decodeWebsocketMessage,
  filterServiceMessagesFromCanisterMessages,
  getNextPollingNonceFromMessages,
} from "./utils/messages";
import { formatClientKey } from "./utils/client";
import { sleep } from "./utils/helpers";

/**
 * The maximum number of messages returned by the **ws_get_messages** method.
 * 
 * Value: `20`
 */
const DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES = 20;
/**
 * Add more messages than the max to check the indexes and limits.
 * @{@link MAX_NUMBER_OF_RETURNED_MESSAGES} + 2
 * 
 * Value: `12`
 */
const SEND_MESSAGES_COUNT = DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES + 2;
/**
 * The interval between sending acks from the canister.
 * Set to a high value to make sure the canister doesn't reset the client while testing other functions.
 * 
 * Value: `300_000` (5 minutes)
 */
const DEFAULT_TEST_SEND_ACK_INTERVAL_MS = 300_000;
/**
 * The interval between keep alive checks in the canister.
 * Set to a high value to make sure the canister doesn't reset the client while testing other functions.
 * 
 * Value: `120_000` (2 minutes)
 */
const DEFAULT_TEST_KEEP_ALIVE_TIMEOUT_MS = 120_000;

let client1Key: ClientKey;
let client2Key: ClientKey;

const assignKeysToClients = async () => {
  if (!client1Key) {
    client1Key = generateClientKey((await client1Data.identity).getPrincipal());
  }
  if (!client2Key) {
    client2Key = generateClientKey((await client2Data.identity).getPrincipal());
  }
};

// testing again canister takes quite a while
jest.setTimeout(60_000);

describe("Canister - ws_open", () => {
  beforeAll(async () => {
    await assignKeysToClients();
  });

  afterAll(async () => {
    await wsWipe();
  });

  it("fails for an anonymous client", async () => {
    const res = await wsOpen({
      canisterId,
      clientActor: anonymousClient,
      clientNonce: getRandomClientNonce(),
    })

    expect(res).toMatchObject<CanisterWsOpenResult>({
      Err: "anonymous principal cannot open a connection",
    });
  });

  it("fails for the registered gateway", async () => {
    const res = await wsOpen({
      canisterId,
      clientActor: gateway1,
      clientNonce: getRandomClientNonce(),
    });

    expect(res).toMatchObject<CanisterWsOpenResult>({
      Err: "caller is the registered gateway which can't open a connection for itself",
    });
  });

  it("should open a connection", async () => {
    const res = await wsOpen({
      canisterId,
      clientActor: client1,
      clientNonce: client1Key.client_nonce,
    });

    expect(res).toMatchObject<CanisterWsOpenResult>({
      Ok: null,
    });

    const msgs = await wsGetMessages({
      fromNonce: 0,
      gatewayActor: gateway1,
    });

    const serviceMessages = filterServiceMessagesFromCanisterMessages(msgs.messages);

    expect(isClientKeyEq(serviceMessages[0].client_key, client1Key)).toEqual(true);
    const openMessage = getServiceMessageContentFromCanisterMessage(serviceMessages[0]);
    expect(openMessage).toMatchObject<WebsocketServiceMessageContent>({
      OpenMessage: expect.any(Object),
    });
    const openMessageContent = (openMessage as { OpenMessage: CanisterOpenMessageContent }).OpenMessage;
    expect(isClientKeyEq(openMessageContent.client_key, client1Key)).toEqual(true);
  });

  it("fails for a client with the same nonce", async () => {
    const res = await wsOpen({
      canisterId,
      clientActor: client1,
      clientNonce: client1Key.client_nonce,
    });

    expect(res).toMatchObject<CanisterWsOpenResult>({
      Err: `client with key ${client1Key.client_principal.toText()}_${client1Key.client_nonce} already has an open connection`,
    });
  });

  it("should open a connection for the same client with a different nonce", async () => {
    const clientKey = {
      ...client1Key,
      client_nonce: getRandomClientNonce(),
    }
    const res = await wsOpen({
      canisterId,
      clientActor: client1,
      clientNonce: clientKey.client_nonce,
    });

    expect(res).toMatchObject<CanisterWsOpenResult>({
      Ok: null,
    });

    const msgs = await wsGetMessages({
      fromNonce: 0,
      gatewayActor: gateway1,
    });

    const serviceMessages = filterServiceMessagesFromCanisterMessages(msgs.messages);
    const serviceMessagesForClient = serviceMessages.filter((msg) => isClientKeyEq(msg.client_key, clientKey));

    const openMessage = getServiceMessageContentFromCanisterMessage(serviceMessagesForClient[0]);
    expect(openMessage).toMatchObject<WebsocketServiceMessageContent>({
      OpenMessage: expect.any(Object),
    });
    const openMessageContent = (openMessage as { OpenMessage: CanisterOpenMessageContent }).OpenMessage;
    expect(isClientKeyEq(openMessageContent.client_key, clientKey)).toEqual(true);
  });
});

describe("Canister - ws_message", () => {
  beforeAll(async () => {
    await assignKeysToClients();

    await initializeCdk({
      maxNumberOfReturnedMessages: DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
      sendAckIntervalMs: DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
      keepAliveDelayMs: DEFAULT_TEST_KEEP_ALIVE_TIMEOUT_MS,
    });

    await wsOpen({
      clientNonce: client1Key.client_nonce,
      canisterId,
      clientActor: client1,
    }, true);
  });

  afterAll(async () => {
    await wsWipe();
  });

  it("fails if client is not registered", async () => {
    const res = await wsMessage({
      message: createWebsocketMessage(client2Key, 0),
      actor: client2,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: `client with principal ${client2Key.client_principal.toText()} doesn't have an open connection`,
    });
  });

  it("fails if client sends a message with a different client key", async () => {
    // first, send a message with a different principal
    const res = await wsMessage({
      message: createWebsocketMessage({ ...client1Key, client_principal: client2Key.client_principal }, 0),
      actor: client1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: `client with principal ${client1Key.client_principal.toText()} has a different key than the one used in the message`,
    });

    // then, send a message with a different nonce
    const res2 = await wsMessage({
      message: createWebsocketMessage({ ...client1Key, client_nonce: getRandomClientNonce() }, 0),
      actor: client1,
    });

    expect(res2).toMatchObject<CanisterWsMessageResult>({
      Err: `client with principal ${client1Key.client_principal.toText()} has a different key than the one used in the message`,
    });
  });

  it("should send a message from a registered client", async () => {
    const res = await wsMessage({
      message: createWebsocketMessage(client1Key, 1),
      actor: client1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Ok: null,
    });
  });

  it("fails if client sends a message with a wrong sequence number", async () => {
    const actualSequenceNumber = 1;
    const expectedSequenceNumber = 2; // first valid message with sequence number 1 was sent in the previous test
    const res = await wsMessage({
      message: createWebsocketMessage(client1Key, actualSequenceNumber),
      actor: client1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: `incoming client's message does not have the expected sequence number. Expected: ${expectedSequenceNumber}, actual: ${actualSequenceNumber}. Client removed.`,
    });

    // check if client has been removed
    const res2 = await wsMessage({
      message: createWebsocketMessage(client1Key, 0), // here the sequence number doesn't matter
      actor: client1,
    });

    expect(res2).toMatchObject<CanisterWsMessageResult>({
      Err: `client with principal ${client1Key.client_principal.toText()} doesn't have an open connection`,
    });
  });

  it("fails if a client sends a wrong service message", async () => {
    // open the connection again
    await wsOpen({
      clientNonce: client1Key.client_nonce,
      canisterId,
      clientActor: client1,
    }, true);

    // wrong content encoding
    const res = await wsMessage({
      message: createWebsocketMessage(client1Key, 1, new Uint8Array([1, 2, 3]), true),
      actor: client1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: expect.stringContaining("Error decoding service message content:"),
    });

    const wrongServiceMessage: WebsocketServiceMessageContent = {
      // the client can only send KeepAliveMessage variant
      AckMessage: {
        last_incoming_sequence_num: BigInt(0),
      }
    };
    const res2 = await wsMessage({
      message: createWebsocketMessage(client1Key, 2, encodeWebsocketServiceMessageContent(wrongServiceMessage), true),
      actor: client1,
    });

    expect(res2).toMatchObject<CanisterWsMessageResult>({
      Err: "Invalid received service message",
    });
  });

  it("should send a service message from a registered client", async () => {
    const clientServiceMessage: WebsocketServiceMessageContent = {
      KeepAliveMessage: {
        last_incoming_sequence_num: BigInt(0),
      },
    };
    const res = await wsMessage({
      message: createWebsocketMessage(client1Key, 3, encodeWebsocketServiceMessageContent(clientServiceMessage), true),
      actor: client1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Ok: null,
    });
  });
});

describe("Canister - ws_get_messages (failures,empty)", () => {
  it("fails if a non registered gateway tries to get messages", async () => {
    const res = await gateway2.ws_get_messages({
      nonce: BigInt(0),
    });

    expect(res).toMatchObject<CanisterWsGetMessagesResult>({
      Err: "caller is not the gateway that has been registered during CDK initialization",
    });
  });

  it("registered gateway should receive empty messages if no messages are available", async () => {
    let res = await gateway1.ws_get_messages({
      nonce: BigInt(0),
    });

    expect(res).toMatchObject<CanisterWsGetMessagesResult>({
      Ok: {
        messages: [],
        cert: new Uint8Array(),
        tree: new Uint8Array(),
      },
    });

    res = await gateway1.ws_get_messages({
      nonce: BigInt(100), // high nonce to make sure the indexes are calculated correctly in the canister
    });

    expect(res).toMatchObject<CanisterWsGetMessagesResult>({
      Ok: {
        messages: [],
        cert: new Uint8Array(),
        tree: new Uint8Array(),
      },
    });
  });
});

describe("Canister - ws_get_messages (receive)", () => {
  beforeAll(async () => {
    await assignKeysToClients();

    await initializeCdk({
      maxNumberOfReturnedMessages: DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
      sendAckIntervalMs: DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
      keepAliveDelayMs: DEFAULT_TEST_KEEP_ALIVE_TIMEOUT_MS,
    });

    await wsOpen({
      clientNonce: client1Key.client_nonce,
      canisterId,
      clientActor: client1,
    }, true);

    // prepare the messages
    const messages = Array.from({ length: SEND_MESSAGES_COUNT }, (_, i) => {
      return { text: `test${i}` };
    });

    await wsSend({
      clientPrincipal: client1Key.client_principal,
      actor: client1,
      messages,
    }, true);
  });

  afterAll(async () => {
    await wsWipe();
  });

  it("registered gateway can receive correct amount of messages", async () => {
    // on open, the canister puts a service message in the queue
    const messagesCount = SEND_MESSAGES_COUNT + 1; // +1 for the open service message
    for (let i = 0; i < messagesCount; i++) {
      const res = await gateway1.ws_get_messages({
        nonce: BigInt(i),
      });

      expect(res).toMatchObject<CanisterWsGetMessagesResult>({
        Ok: {
          messages: expect.any(Array),
          cert: expect.any(Uint8Array),
          tree: expect.any(Uint8Array),
        },
      });

      const messagesResult = (res as { Ok: CanisterOutputCertifiedMessages }).Ok;
      expect(messagesResult.messages.length).toEqual(
        messagesCount - i > DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES
          ? DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES
          : messagesCount - i
      );
    }

    // try to get more messages than available
    const res = await gateway1.ws_get_messages({
      nonce: BigInt(messagesCount),
    });

    expect(res).toMatchObject<CanisterWsGetMessagesResult>({
      Ok: {
        messages: [],
        cert: expect.any(Uint8Array),
        tree: expect.any(Uint8Array),
      },
    });
  });

  it("registered gateway can receive certified messages", async () => {
    // first batch of messages
    const firstBatchRes = await gateway1.ws_get_messages({
      nonce: BigInt(1), // skip the case in which the gateway restarts polling from the beginning (tested below)
    });

    const firstBatchMessagesResult = (firstBatchRes as { Ok: CanisterOutputCertifiedMessages }).Ok;
    expect(firstBatchMessagesResult.messages.length).toEqual(DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES);

    let expectedSequenceNumber = 2; // first is the service open message and the number is incremented before sending
    let i = 0;
    for (const message of firstBatchMessagesResult.messages) {
      expect(isClientKeyEq(message.client_key, client1Key)).toEqual(true);
      const websocketMessage = decodeWebsocketMessage(new Uint8Array(message.content));
      expect(websocketMessage).toMatchObject<WebsocketMessage>({
        client_key: expect.any(Object),
        content: expect.any(Uint8Array),
        sequence_num: BigInt(expectedSequenceNumber),
        timestamp: expect.any(BigInt),
        is_service_message: false,
      });
      expect(isClientKeyEq(websocketMessage.client_key, client1Key)).toEqual(true);
      expect(IDL.decode([IDL.Record({ 'text': IDL.Text })], websocketMessage.content as Uint8Array)).toEqual([{ text: `test${i}` }]);

      // check the certification
      await expect(
        isValidCertificate(
          canisterId,
          firstBatchMessagesResult.cert as Uint8Array,
          firstBatchMessagesResult.tree as Uint8Array,
          commonAgent
        )
      ).resolves.toEqual(true);
      await expect(
        isMessageBodyValid(
          message.key,
          message.content as Uint8Array,
          firstBatchMessagesResult.tree as Uint8Array,
        )
      ).resolves.toEqual(true);

      expectedSequenceNumber++;
      i++;
    }

    const nextPollingNonce = getNextPollingNonceFromMessages(firstBatchMessagesResult.messages);

    // second batch of messages, starting from the last nonce of the first batch
    const secondBatchRes = await gateway1.ws_get_messages({
      nonce: BigInt(nextPollingNonce),
    });

    const secondBatchMessagesResult = (secondBatchRes as { Ok: CanisterOutputCertifiedMessages }).Ok;
    expect(secondBatchMessagesResult.messages.length).toEqual(SEND_MESSAGES_COUNT - DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES); // remaining from SEND_MESSAGES_COUNT

    for (const message of secondBatchMessagesResult.messages) {
      expect(isClientKeyEq(message.client_key, client1Key)).toEqual(true);
      const websocketMessage = decodeWebsocketMessage(new Uint8Array(message.content));
      expect(websocketMessage).toMatchObject<WebsocketMessage>({
        client_key: expect.any(Object),
        content: expect.any(Uint8Array),
        sequence_num: BigInt(expectedSequenceNumber),
        timestamp: expect.any(BigInt),
        is_service_message: false,
      });
      expect(isClientKeyEq(websocketMessage.client_key, client1Key)).toEqual(true);
      expect(IDL.decode([IDL.Record({ 'text': IDL.Text })], websocketMessage.content as Uint8Array)).toEqual([{ text: `test${i}` }]);

      // check the certification
      await expect(
        isValidCertificate(
          canisterId,
          secondBatchMessagesResult.cert as Uint8Array,
          secondBatchMessagesResult.tree as Uint8Array,
          commonAgent
        )
      ).resolves.toEqual(true);
      await expect(
        isMessageBodyValid(
          message.key,
          message.content as Uint8Array,
          secondBatchMessagesResult.tree as Uint8Array,
        )
      ).resolves.toEqual(true);

      expectedSequenceNumber++;
      i++;
    }
  });

  it("registered gateway can poll messages after restart", async () => {
    const batchRes = await gateway1.ws_get_messages({
      nonce: BigInt(0), // start polling from the beginning, as if the gateway restarted
    });

    // we expect that the messages returned are the last MAX_NUMBER_OF_RETURNED_MESSAGES
    const messagesResult = (batchRes as { Ok: CanisterOutputCertifiedMessages }).Ok;
    expect(messagesResult.messages.length).toEqual(DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES);

    let expectedSequenceNumber = SEND_MESSAGES_COUNT - DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES + 1 + 1; // +1 for the service open message +1 because the seq num is incremented before sending
    let i = SEND_MESSAGES_COUNT - DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES;
    for (const message of messagesResult.messages) {
      expect(isClientKeyEq(message.client_key, client1Key)).toEqual(true);
      const websocketMessage = decodeWebsocketMessage(new Uint8Array(message.content));
      expect(websocketMessage).toMatchObject<WebsocketMessage>({
        client_key: expect.any(Object),
        content: expect.any(Uint8Array),
        sequence_num: BigInt(expectedSequenceNumber),
        timestamp: expect.any(BigInt),
        is_service_message: false,
      });
      expect(isClientKeyEq(websocketMessage.client_key, client1Key)).toEqual(true);
      expect(IDL.decode([IDL.Record({ 'text': IDL.Text })], websocketMessage.content as Uint8Array)).toEqual([{ text: `test${i}` }]);

      // check the certification
      await expect(
        isValidCertificate(
          canisterId,
          messagesResult.cert as Uint8Array,
          messagesResult.tree as Uint8Array,
          commonAgent
        )
      ).resolves.toEqual(true);
      await expect(
        isMessageBodyValid(
          message.key,
          message.content as Uint8Array,
          messagesResult.tree as Uint8Array,
        )
      ).resolves.toEqual(true);

      expectedSequenceNumber++;
      i++;
    }
  });
});

describe("Canister - ws_close", () => {
  beforeAll(async () => {
    await assignKeysToClients();

    await initializeCdk({
      maxNumberOfReturnedMessages: DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
      sendAckIntervalMs: DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
      keepAliveDelayMs: DEFAULT_TEST_KEEP_ALIVE_TIMEOUT_MS,
    });

    await wsOpen({
      clientNonce: client1Key.client_nonce,
      canisterId,
      clientActor: client1,
    }, true);
  });

  afterAll(async () => {
    await wsWipe();
  });

  it("fails if gateway is not registered", async () => {
    const res = await wsClose({
      clientKey: client1Key,
      gatewayActor: gateway2,
    });

    expect(res).toMatchObject<CanisterWsCloseResult>({
      Err: "caller is not the gateway that has been registered during CDK initialization",
    });
  });

  it("fails if client is not registered", async () => {
    const res = await wsClose({
      clientKey: client2Key,
      gatewayActor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsCloseResult>({
      Err: `client with key ${formatClientKey(client2Key)} doesn't have an open connection`,
    });
  });

  it("should close the websocket for a registered client", async () => {
    const res = await wsClose({
      clientKey: client1Key,
      gatewayActor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsCloseResult>({
      Ok: null,
    });
  });
});

describe("Canister - ws_send", () => {
  beforeAll(async () => {
    await assignKeysToClients();

    await initializeCdk({
      maxNumberOfReturnedMessages: DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
      sendAckIntervalMs: DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
      keepAliveDelayMs: DEFAULT_TEST_KEEP_ALIVE_TIMEOUT_MS,
    });

    await wsOpen({
      clientNonce: client1Key.client_nonce,
      canisterId,
      clientActor: client1,
    }, true);
  });

  afterAll(async () => {
    await wsWipe();
  });

  it("fails if sending a message to a non registered client", async () => {
    const res = await wsSend({
      clientPrincipal: client2Key.client_principal,
      actor: client1,
      messages: [{ text: "test" }],
    });

    expect(res).toMatchObject<CanisterWsSendResult>({
      Err: `client with principal ${client2Key.client_principal.toText()} doesn't have an open connection`,
    });
  });

  it("should send a message to a registered client", async () => {
    const res = await wsSend({
      clientPrincipal: client1Key.client_principal,
      actor: client1,
      messages: [{ text: "test" }],
    });

    expect(res).toMatchObject<CanisterWsSendResult>({
      Ok: null,
    });
  });
});

describe("Messages acknowledgement", () => {
  beforeAll(async () => {
    await assignKeysToClients();
  });

  afterEach(async () => {
    await wsWipe();
  });

  it("client should receive ack messages", async () => {
    const sendAckIntervalMs = 10_000; // 10 seconds
    const keepAliveDelayMs = 5_000; // 5 seconds
    await initializeCdk({
      maxNumberOfReturnedMessages: DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
      sendAckIntervalMs,
      keepAliveDelayMs,
    });

    await wsOpen({
      clientNonce: client1Key.client_nonce,
      canisterId,
      clientActor: client1,
    }, true);

    const res = await gateway1.ws_get_messages({
      nonce: BigInt(1), // skip the service open message
    });

    expect(res).toMatchObject<CanisterWsGetMessagesResult>({
      Ok: {
        messages: [],
        cert: new Uint8Array(),
        tree: new Uint8Array(),
      },
    });

    await wsMessage({
      actor: client1,
      message: createWebsocketMessage(client1Key, 1),
    }, true);

    // sleep for 10 seconds, which is more than the sendAckIntervalMs due to the previous calls
    // so we are sure that the CDK has sent an ack
    await sleep(sendAckIntervalMs);

    const res2 = await gateway1.ws_get_messages({
      nonce: BigInt(1),
    });

    expect(res2).toMatchObject<CanisterWsGetMessagesResult>({
      Ok: {
        messages: expect.any(Array),
        cert: expect.any(Uint8Array),
        tree: expect.any(Uint8Array),
      },
    });
    const messagesResult = (res2 as { Ok: CanisterOutputCertifiedMessages }).Ok;
    expect(messagesResult.messages.length).toEqual(1);

    const ackMessage = messagesResult.messages[0];
    expect(isClientKeyEq(ackMessage.client_key, client1Key)).toEqual(true);
    expect(getServiceMessageContentFromCanisterMessage(ackMessage)).toMatchObject<WebsocketServiceMessageContent>({
      AckMessage: {
        last_incoming_sequence_num: BigInt(1),
      }
    });

    // check if the certification is correct
    await expect(
      isValidCertificate(
        canisterId,
        messagesResult.cert as Uint8Array,
        messagesResult.tree as Uint8Array,
        commonAgent
      )
    ).resolves.toEqual(true);
    await expect(
      isMessageBodyValid(
        ackMessage.key,
        ackMessage.content as Uint8Array,
        messagesResult.tree as Uint8Array,
      )
    ).resolves.toEqual(true);
  });

  it("client is removed if keep alive timeout is reached", async () => {
    const sendAckIntervalMs = 10_000; // 10 seconds
    const keepAliveDelayMs = 5_000; // 5 seconds
    await initializeCdk({
      maxNumberOfReturnedMessages: DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
      sendAckIntervalMs,
      keepAliveDelayMs,
    });

    await wsOpen({
      clientNonce: client1Key.client_nonce,
      canisterId,
      clientActor: client1,
    }, true);

    await sleep(sendAckIntervalMs);

    const res = await gateway1.ws_get_messages({
      nonce: BigInt(1), // skip the service open message
    });

    const messagesResult = (res as { Ok: CanisterOutputCertifiedMessages }).Ok;
    expect(messagesResult.messages.length).toEqual(1);
    // just check if the received message is a service message and belongs to the client
    expect(isClientKeyEq(messagesResult.messages[0].client_key, client1Key)).toEqual(true);
    const websocketMessage = getWebsocketMessageFromCanisterMessage(messagesResult.messages[0]);
    expect(websocketMessage.is_service_message).toEqual(true);

    await sleep(keepAliveDelayMs);

    // to check if the client is not registered anymore, we try to send a message
    const res2 = await wsSend({
      clientPrincipal: client1Key.client_principal,
      actor: client1,
      messages: [{ text: "test" }],
    });

    expect(res2).toMatchObject<CanisterWsSendResult>({
      Err: `client with principal ${client1Key.client_principal.toText()} doesn't have an open connection`,
    });
  });

  it("client is not removed if it sends a keep alive before timeout", async () => {
    const sendAckIntervalMs = 15_000; // 15 seconds
    const keepAliveDelayMs = 5_000; // 5 seconds
    await initializeCdk({
      maxNumberOfReturnedMessages: DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
      sendAckIntervalMs,
      keepAliveDelayMs,
    });

    await wsOpen({
      clientNonce: client1Key.client_nonce,
      canisterId,
      clientActor: client1,
    }, true);

    await sleep(sendAckIntervalMs);

    let res = await gateway1.ws_get_messages({
      nonce: BigInt(1), // skip the service open message
    });
    let messagesResult = (res as { Ok: CanisterOutputCertifiedMessages }).Ok;
    // the queue contains only the first ack message
    expect(messagesResult.messages.length).toEqual(1);
    let ackMessage = messagesResult.messages[0];
    expect(isClientKeyEq(ackMessage.client_key, client1Key)).toEqual(true);
    let websocketMessage = getWebsocketMessageFromCanisterMessage(ackMessage);
    expect(websocketMessage.is_service_message).toEqual(true);
    expect(websocketMessage.sequence_num).toEqual(2);
    let serviceMessageContent = decodeWebsocketServiceMessageContent(websocketMessage.content as Uint8Array);
    expect(serviceMessageContent).toMatchObject<WebsocketServiceMessageContent>({
      AckMessage: {
        last_incoming_sequence_num: BigInt(0),
      },
    });

    // send the keep alive message
    const keepAliveMessage: WebsocketServiceMessageContent = {
      KeepAliveMessage: {
        last_incoming_sequence_num: BigInt(1), // ignored in the CDK
      },
    };
    await wsMessage({
      actor: client1,
      message: createWebsocketMessage(client1Key, 1, encodeWebsocketServiceMessageContent(keepAliveMessage), true),
    }, true);

    // wait for the canister to check if the client has sent the keep alive
    await sleep(keepAliveDelayMs);

    // send a message to the canister to see the sequence number increasing in the ack message
    // and be sure that the client can still send messages
    await wsMessage({
      actor: client1,
      message: createWebsocketMessage(client1Key, 2),
    }, true);

    // wait to receive the next acknowledgement
    await sleep(sendAckIntervalMs);

    res = await gateway1.ws_get_messages({
      nonce: BigInt(2), // skip the service open message and the first service ack message
    });

    messagesResult = (res as { Ok: CanisterOutputCertifiedMessages }).Ok;
    // the fetched queue only contains the second ack message
    expect(messagesResult.messages.length).toEqual(1);
    ackMessage = messagesResult.messages[0];
    expect(isClientKeyEq(ackMessage.client_key, client1Key)).toEqual(true);
    websocketMessage = getWebsocketMessageFromCanisterMessage(ackMessage);
    expect(websocketMessage.is_service_message).toEqual(true);
    expect(websocketMessage.sequence_num).toEqual(3);
    serviceMessageContent = decodeWebsocketServiceMessageContent(websocketMessage.content as Uint8Array);
    expect(serviceMessageContent).toMatchObject<WebsocketServiceMessageContent>({
      AckMessage: {
        last_incoming_sequence_num: BigInt(2), // as expected, the canister acks both the messages that we've sent
      },
    });
  });

  it("client is not removed if it connects while canister is waiting for keep alive", async () => {
    const sendAckIntervalMs = 15_000; // 15 seconds
    const keepAliveDelayMs = 10_000; // 10 seconds, to make sure the canister is waiting for keep alive when the client connects
    await initializeCdk({
      maxNumberOfReturnedMessages: DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
      sendAckIntervalMs,
      keepAliveDelayMs,
    });

    // make sure the canister is waiting for keep alive
    await sleep(sendAckIntervalMs);

    await wsOpen({
      clientNonce: client1Key.client_nonce,
      canisterId,
      clientActor: client1,
    }, true);

    let res = await gateway1.ws_get_messages({
      nonce: BigInt(1), // skip the service open message
    });
    let messagesResult = (res as { Ok: CanisterOutputCertifiedMessages }).Ok;
    expect(messagesResult.messages.length).toEqual(0); // client doesn't expect any other messages at this point

    // send a message to the canister to see the sequence number increasing in the ack message
    // ad verify that the client is still connected to the canister
    const wsMessageRes = await wsMessage({
      actor: client1,
      message: createWebsocketMessage(client1Key, 1),
    });
    expect(wsMessageRes).toMatchObject<CanisterWsMessageResult>({
      Ok: null,
    });

    // wait to for the keep alive timeout to expire
    await sleep(keepAliveDelayMs);
    // wait for the canister to send the next ack
    await sleep(sendAckIntervalMs - keepAliveDelayMs);

    res = await gateway1.ws_get_messages({
      nonce: BigInt(1), // skip the service open message
    });

    messagesResult = (res as { Ok: CanisterOutputCertifiedMessages }).Ok;
    expect(messagesResult.messages.length).toEqual(1);

    const ackMessage = messagesResult.messages[0];
    expect(isClientKeyEq(ackMessage.client_key, client1Key)).toEqual(true);
    const websocketMessage = getWebsocketMessageFromCanisterMessage(ackMessage);
    expect(websocketMessage.is_service_message).toEqual(true);
    expect(websocketMessage.sequence_num).toEqual(2); // first message is skipped and sequence number starts from 1
    const serviceMessageContent = decodeWebsocketServiceMessageContent(websocketMessage.content as Uint8Array);
    expect(serviceMessageContent).toMatchObject<WebsocketServiceMessageContent>({
      AckMessage: {
        last_incoming_sequence_num: BigInt(1),
      },
    });
  })
});
