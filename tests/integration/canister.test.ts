import { IDL } from "@dfinity/candid";
import { Principal } from "@dfinity/principal";
import { Cbor } from "@dfinity/agent";
import {
  canisterId,
  client1,
  client2,
  commonAgent,
  gateway1,
  gateway2,
} from "./utils/actors";
import { getKeyPair, getMessageSignature } from "./utils/crypto";
import {
  getWebsocketMessage,
  isMessageBodyValid,
  isValidCertificate,
  wsClose,
  wsMessage,
  wsOpen,
  wsRegister,
  wsSend,
  wsWipe,
} from "./utils/api";
import type {
  CanisterOutputCertifiedMessages,
  CanisterWsCloseResult,
  CanisterWsGetMessagesResult,
  CanisterWsMessageResult,
  CanisterWsOpenResult,
  CanisterWsRegisterResult,
  CanisterWsSendResult,
} from "../src/declarations/test_canister/test_canister.did";
import type { WebsocketMessage } from "./utils/api";

const MAX_NUMBER_OF_RETURNED_MESSAGES = 10; // set in the CDK
const SEND_MESSAGES_COUNT = MAX_NUMBER_OF_RETURNED_MESSAGES + 2; // test with more messages to check the indexes and limits
const MAX_GATEWAY_KEEP_ALIVE_TIME_MS = 15_000; // set in the CDK

let client1KeyPair: { publicKey: Uint8Array; secretKey: Uint8Array | string; };
let client2KeyPair: { publicKey: Uint8Array; secretKey: Uint8Array | string; };

// the status index used by the gateway to send a keep-alive message
let gatewayStatusIndex = 0;

const sendGatewayStatusMessage = async (index?: number) => {
  const statusIndex = index !== undefined ? index : gatewayStatusIndex;

  await wsMessage({
    message: {
      IcWebSocketGatewayStatus: {
        status_index: BigInt(statusIndex),
      }
    },
    actor: gateway1,
  }, true);

  gatewayStatusIndex += 1;
};

const assignKeyPairsToClients = async () => {
  if (!client1KeyPair) {
    client1KeyPair = await getKeyPair();
  }
  if (!client2KeyPair) {
    client2KeyPair = await getKeyPair();
  }
};

// testing again canister takes quite a while
jest.setTimeout(60_000);

describe("Canister - ws_register", () => {
  beforeAll(async () => {
    await assignKeyPairsToClients();
  });

  afterAll(async () => {
    await wsWipe(gateway1);
  });

  it("should register a client", async () => {
    const res = await wsRegister({
      clientActor: client1,
      clientKey: client1KeyPair.publicKey,
    });

    expect(res).toMatchObject<CanisterWsRegisterResult>({
      Ok: null,
    });
  });
});

describe("Canister - ws_open", () => {
  beforeAll(async () => {
    await assignKeyPairsToClients();

    await wsRegister({
      clientActor: client1,
      clientKey: client1KeyPair.publicKey,
    }, true);
  });

  afterAll(async () => {
    await wsWipe(gateway1);
  });

  beforeEach(async () => {
    await sendGatewayStatusMessage();
  });

  it("fails for a gateway which is not registered", async () => {
    const res = await wsOpen({
      clientPublicKey: client1KeyPair.publicKey,
      clientSecretKey: client1KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway2,
    });

    expect(res).toMatchObject<CanisterWsOpenResult>({
      Err: "caller is not the gateway that has been registered during CDK initialization",
    });
  });

  it("fails if a registered gateway relays a wrong first message", async () => {
    // empty message
    let content = Cbor.encode({})
    let res = await gateway1.ws_open({
      content: new Uint8Array(content),
      sig: await getMessageSignature(content, client1KeyPair.secretKey),
    });
    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "missing field `client_key`",
    });

    // with client_key
    content = Cbor.encode({
      client_key: client1KeyPair.publicKey,
    });
    res = await gateway1.ws_open({
      content: new Uint8Array(content),
      sig: await getMessageSignature(content, client1KeyPair.secretKey),
    });
    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "missing field `canister_id`",
    });
  });

  it("fails for a client which is not registered", async () => {
    const res = await wsOpen({
      clientPublicKey: client2KeyPair.publicKey,
      clientSecretKey: client2KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsOpenResult>({
      Err: "client's public key has not been previously registered by client",
    });
  });

  it("fails for an invalid signature", async () => {
    // sign message with client2 secret key but send client1 public key
    const res = await wsOpen({
      clientPublicKey: client1KeyPair.publicKey,
      clientSecretKey: client2KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsOpenResult>({
      Err: "Signature doesn't verify",
    });
  });

  it("fails for a client which is not registered after the gateway has been reset", async () => {
    await sendGatewayStatusMessage(0);

    const res = await wsOpen({
      clientPublicKey: client2KeyPair.publicKey,
      clientSecretKey: client2KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsOpenResult>({
      Err: "client's public key has not been previously registered by client",
    });
  });

  it("fails for a client which is registered, but after the gateway increased the status index by two and then been reset", async () => {
    // reset the canister state from the previous test
    await wsWipe(gateway1);
    // register the client again
    await wsRegister({
      clientActor: client1,
      clientKey: client1KeyPair.publicKey,
    }, true);

    // send two status messages to make the client key shift out of the tmp ones
    await sendGatewayStatusMessage();
    await sendGatewayStatusMessage();

    // reset the gateway on the canister
    await sendGatewayStatusMessage(0);

    const res = await wsOpen({
      clientPublicKey: client1KeyPair.publicKey,
      clientSecretKey: client1KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsOpenResult>({
      Err: "client's public key has not been previously registered by client",
    });
  });

  it("should open the websocket for a registered client after gateway has been reset", async () => {
    // reset the canister state from the previous test
    await wsWipe(gateway1);
    // register the client again
    await wsRegister({
      clientActor: client1,
      clientKey: client1KeyPair.publicKey,
    }, true);

    // reset the gateway on the canister
    await sendGatewayStatusMessage(0);

    const res = await wsOpen({
      clientPublicKey: client1KeyPair.publicKey,
      clientSecretKey: client1KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsOpenResult>({
      Ok: {
        client_key: client1KeyPair.publicKey,
        canister_id: Principal.fromText(canisterId),
        nonce: BigInt(0),
      },
    });
  });

  it("should open the websocket for a registered client", async () => {
    // reset the canister state from the previous test
    await wsWipe(gateway1);
    // setup the canister state again
    await sendGatewayStatusMessage();
    await wsRegister({
      clientActor: client1,
      clientKey: client1KeyPair.publicKey,
    }, true);

    // open the websocket
    const res = await wsOpen({
      clientPublicKey: client1KeyPair.publicKey,
      clientSecretKey: client1KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsOpenResult>({
      Ok: {
        client_key: client1KeyPair.publicKey,
        canister_id: Principal.fromText(canisterId),
        nonce: BigInt(0),
      },
    });
  });
});

describe("Canister - ws_message", () => {
  beforeAll(async () => {
    await assignKeyPairsToClients();

    await wsRegister({
      clientActor: client1,
      clientKey: client1KeyPair.publicKey,
    }, true);

    await wsOpen({
      clientPublicKey: client1KeyPair.publicKey,
      clientSecretKey: client1KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway1,
    }, true);
  });

  afterAll(async () => {
    await wsWipe(gateway1);
  });

  beforeEach(async () => {
    await sendGatewayStatusMessage();
  });

  it("fails if a non registered gateway sends an IcWebSocketEstablished message", async () => {
    const res = await wsMessage({
      message: {
        IcWebSocketEstablished: client1KeyPair.publicKey,
      },
      actor: gateway2,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "caller is not the gateway that has been registered during CDK initialization",
    });
  });

  it("fails if a non registered gateway sends a RelayedByGateway message", async () => {
    const content = getWebsocketMessage(client1KeyPair.publicKey, 0);
    const res = await wsMessage({
      message: {
        RelayedByGateway: {
          content,
          sig: await getMessageSignature(content, client1KeyPair.secretKey),
        }
      },
      actor: gateway2,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "caller is not the gateway that has been registered during CDK initialization",
    });
  });

  it("fails if a non registered client sends a DirectlyFromClient message", async () => {
    const message = IDL.encode([IDL.Record({ 'text': IDL.Text })], [{ text: "pong" }]);
    const res = await wsMessage({
      message: {
        DirectlyFromClient: {
          client_key: client2KeyPair.publicKey,
          message: new Uint8Array(message),
        }
      },
      actor: client2,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "client is not registered, call ws_register first",
    });
  });

  it("fails if a non registered client sends a DirectlyFromClient message using a registered client key", async () => {
    const message = IDL.encode([IDL.Record({ 'text': IDL.Text })], [{ text: "pong" }]);
    const res = await wsMessage({
      message: {
        DirectlyFromClient: {
          client_key: client1KeyPair.publicKey,
          message: new Uint8Array(message),
        }
      },
      actor: client2,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "caller is not the same that registered the public key",
    });
  });

  it("fails if a registered gateway sends an IcWebSocketEstablished message for a non registered client", async () => {
    const res = await wsMessage({
      message: {
        IcWebSocketEstablished: client2KeyPair.publicKey,
      },
      actor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "client's public key has not been previously registered by client",
    });
  });

  it("fails if a registered gateway sends a wrong RelayedByGateway message", async () => {
    // empty message
    let content = Cbor.encode({});
    let res = await wsMessage({
      message: {
        RelayedByGateway: {
          content: new Uint8Array(content),
          sig: await getMessageSignature(content, client2KeyPair.secretKey),
        },
      },
      actor: gateway1,
    });
    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "missing field `client_key`",
    });

    // with client_key
    content = Cbor.encode({
      client_key: client1KeyPair.publicKey,
    });
    res = await wsMessage({
      message: {
        RelayedByGateway: {
          content: new Uint8Array(content),
          sig: await getMessageSignature(content, client2KeyPair.secretKey),
        },
      },
      actor: gateway1,
    });
    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "missing field `sequence_num`",
    });

    // with client_key, sequence_num
    content = Cbor.encode({
      client_key: client1KeyPair.publicKey,
      sequence_num: 0,
    });
    res = await wsMessage({
      message: {
        RelayedByGateway: {
          content: new Uint8Array(content),
          sig: await getMessageSignature(content, client2KeyPair.secretKey),
        },
      },
      actor: gateway1,
    });
    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "missing field `timestamp`",
    });

    // with client_key, sequence_num, timestamp
    content = Cbor.encode({
      client_key: client1KeyPair.publicKey,
      sequence_num: 0,
      timestamp: Date.now(),
    });
    res = await wsMessage({
      message: {
        RelayedByGateway: {
          content: new Uint8Array(content),
          sig: await getMessageSignature(content, client2KeyPair.secretKey),
        },
      },
      actor: gateway1,
    });
    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "missing field `message`",
    });
  });

  it("fails if a registered gateway sends a RelayedByGateway message with an invalid signature", async () => {
    const content = getWebsocketMessage(client1KeyPair.publicKey, 0);
    const res = await wsMessage({
      message: {
        RelayedByGateway: {
          content,
          sig: await getMessageSignature(content, client2KeyPair.secretKey),
        }
      },
      actor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "Signature doesn't verify",
    });
  });

  it("fails if registered gateway sends a RelayedByGateway message with a wrong sequence number", async () => {
    const appMessage = IDL.Record({ 'text': IDL.Text }).encodeValue({ text: "pong" });
    let content = getWebsocketMessage(client1KeyPair.publicKey, 1, appMessage);
    let res = await wsMessage({
      message: {
        RelayedByGateway: {
          content,
          sig: await getMessageSignature(content, client1KeyPair.secretKey),
        }
      },
      actor: gateway1,
    });
    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "incoming client's message relayed from WS Gateway does not have the expected sequence number",
    });

    // send a correct message to increase the sequence number
    content = getWebsocketMessage(client1KeyPair.publicKey, 0, appMessage);
    res = await wsMessage({
      message: {
        RelayedByGateway: {
          content,
          sig: await getMessageSignature(content, client1KeyPair.secretKey),
        }
      },
      actor: gateway1,
    });
    expect(res).toMatchObject<CanisterWsMessageResult>({
      Ok: null,
    });

    // send a message with the old sequence number
    content = getWebsocketMessage(client1KeyPair.publicKey, 0, appMessage);
    res = await wsMessage({
      message: {
        RelayedByGateway: {
          content,
          sig: await getMessageSignature(content, client1KeyPair.secretKey),
        }
      },
      actor: gateway1,
    });
    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "incoming client's message relayed from WS Gateway does not have the expected sequence number",
    });

    // send a message with a sequence number that is too high
    content = getWebsocketMessage(client1KeyPair.publicKey, 2, appMessage);
    res = await wsMessage({
      message: {
        RelayedByGateway: {
          content,
          sig: await getMessageSignature(content, client1KeyPair.secretKey),
        }
      },
      actor: gateway1,
    });
    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "incoming client's message relayed from WS Gateway does not have the expected sequence number",
    });
  });

  it("fails if a registered gateway sends a RelayedByGateway for a registered client that doesn't have open connection", async () => {
    // register another client, but don't call ws_open for it
    await wsRegister({
      clientActor: client2,
      clientKey: client2KeyPair.publicKey,
    }, true);

    const content = getWebsocketMessage(client2KeyPair.publicKey, 0);
    const res = await wsMessage({
      message: {
        RelayedByGateway: {
          content,
          sig: await getMessageSignature(content, client2KeyPair.secretKey),
        }
      },
      actor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "expected incoming message num not initialized for client",
    });
  });

  it("fails if registered gateway sends a DirectlyFromClient message", async () => {
    const message = IDL.encode([IDL.Record({ 'text': IDL.Text })], [{ text: "pong" }]);
    const res = await wsMessage({
      message: {
        DirectlyFromClient: {
          client_key: client1KeyPair.publicKey,
          message: new Uint8Array(message),
        }
      },
      actor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "caller is not the same that registered the public key",
    });
  });

  it("a registered gateway should send a message (IcWebSocketEstablished) for a registered client", async () => {
    const res = await wsMessage({
      message: {
        IcWebSocketEstablished: client1KeyPair.publicKey,
      },
      actor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Ok: null,
    });
  });

  it("a registered gateway should send a message (RelayedByGateway) for a registered client", async () => {
    const appMessage = IDL.encode([IDL.Record({ 'text': IDL.Text })], [{ text: "pong" }]);
    // the message with sequence number 0 has been sent in a previous test, so we send a message with sequence number 1
    const content = getWebsocketMessage(client1KeyPair.publicKey, 1, appMessage);
    const res = await wsMessage({
      message: {
        RelayedByGateway: {
          content,
          sig: await getMessageSignature(content, client1KeyPair.secretKey),
        }
      },
      actor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Ok: null,
    });
  });

  it("a registered client should send a message (DirectlyFromClient)", async () => {
    const message = IDL.encode([IDL.Record({ 'text': IDL.Text })], [{ text: "pong" }]);
    const res = await wsMessage({
      message: {
        DirectlyFromClient: {
          client_key: client1KeyPair.publicKey,
          message: new Uint8Array(message),
        }
      },
      actor: client1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Ok: null,
    });
  });
});

describe("Canister - ws_get_messages (failures,empty)", () => {
  beforeAll(async () => {
    await assignKeyPairsToClients();

    await wsRegister({
      clientActor: client1,
      clientKey: client1KeyPair.publicKey,
    }, true);

    await wsOpen({
      clientPublicKey: client1KeyPair.publicKey,
      clientSecretKey: client1KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway1,
    }, true);

    await commonAgent.fetchRootKey();
  });

  afterAll(async () => {
    await wsWipe(gateway1);
  });

  beforeEach(async () => {
    await sendGatewayStatusMessage();
  });

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

describe("Canister - ws_message (gateway status)", () => {
  beforeAll(async () => {
    await assignKeyPairsToClients();

    await wsRegister({
      clientActor: client1,
      clientKey: client1KeyPair.publicKey,
    }, true);

    await wsOpen({
      clientPublicKey: client1KeyPair.publicKey,
      clientSecretKey: client1KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway1,
    }, true);

    await wsSend({
      clientPublicKey: client1KeyPair.publicKey,
      actor: client1,
      message: { text: "test" },
    }, true);
  });

  afterAll(async () => {
    await wsWipe(gateway1);
  });

  it("fails if a non registered gateway sends an IcWebSocketGatewayStatus message", async () => {
    const res = await wsMessage({
      message: {
        IcWebSocketGatewayStatus: {
          status_index: BigInt(1),
        },
      },
      actor: gateway2,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "caller is not the gateway that has been registered during CDK initialization",
    });
  });

  it("registered gateway should update the status index", async () => {
    const res = await wsMessage({
      message: {
        IcWebSocketGatewayStatus: {
          status_index: BigInt(2), // set it high to test behavior for indexes behind the current one
        },
      },
      actor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Ok: null,
    });
  });

  it("fails if a registered gateway sends an IcWebSocketGatewayStatus with a wrong status index (equal to current)", async () => {
    const res = await wsMessage({
      message: {
        IcWebSocketGatewayStatus: {
          status_index: BigInt(2),
        },
      },
      actor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "Gateway status index is equal to or behind the current one",
    });
  });

  it("fails if a registered gateway sends an IcWebSocketGatewayStatus with a wrong status index (behind the current)", async () => {
    const res = await wsMessage({
      message: {
        IcWebSocketGatewayStatus: {
          status_index: BigInt(1),
        },
      },
      actor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Err: "Gateway status index is equal to or behind the current one",
    });
  });

  it("registered gateway should disconnect after maximum time", async () => {
    let res = await gateway1.ws_get_messages({
      nonce: BigInt(0),
    });

    expect(res).toMatchObject<CanisterWsGetMessagesResult>({
      Ok: {
        messages: expect.any(Array),
        cert: expect.any(Uint8Array),
        tree: expect.any(Uint8Array),
      },
    });
    expect((res as { Ok: CanisterOutputCertifiedMessages }).Ok.messages.length).toEqual(1);

    // wait for the maximum time the gateway can send a status message,
    // so that the internal canister state is reset
    // double the time to make sure the canister state is reset
    await new Promise((resolve) => setTimeout(resolve, 2 * MAX_GATEWAY_KEEP_ALIVE_TIME_MS));

    // check if messages have been deleted
    res = await gateway1.ws_get_messages({
      nonce: BigInt(0),
    });
    expect(res).toMatchObject<CanisterWsGetMessagesResult>({
      Ok: {
        messages: [],
        cert: expect.any(Uint8Array),
        tree: expect.any(Uint8Array),
      },
    });

    // check if registered client has been deleted
    const sendRes = await wsSend({
      clientPublicKey: client1KeyPair.publicKey,
      actor: client1,
      message: { text: "test" },
    });
    expect(sendRes).toMatchObject<CanisterWsSendResult>({
      Err: "client's public key has not been previously registered by client",
    });
  });

  it("registered gateway should reconnect by resetting the status index", async () => {
    let res = await wsMessage({
      message: {
        IcWebSocketGatewayStatus: {
          status_index: BigInt(0),
        },
      },
      actor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Ok: null,
    });

    res = await wsMessage({
      message: {
        IcWebSocketGatewayStatus: {
          status_index: BigInt(1),
        },
      },
      actor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsMessageResult>({
      Ok: null,
    });
  });

  it("registered gateway should reconnect before maximum time", async () => {
    // reconnect the client
    await wsRegister({
      clientActor: client1,
      clientKey: client1KeyPair.publicKey,
    }, true);

    await wsOpen({
      clientPublicKey: client1KeyPair.publicKey,
      clientSecretKey: client1KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway1,
    }, true);

    // send a test message from the canister to check if the internal state is reset
    await wsSend({
      clientPublicKey: client1KeyPair.publicKey,
      actor: client1,
      message: { text: "test" },
    }, true);

    // check if the canister has the message in the queue
    let messagesRes = await gateway1.ws_get_messages({
      nonce: BigInt(0),
    });
    expect(messagesRes).toMatchObject<CanisterWsGetMessagesResult>({
      Ok: {
        messages: expect.any(Array),
        cert: expect.any(Uint8Array),
        tree: expect.any(Uint8Array),
      },
    });
    expect((messagesRes as { Ok: CanisterOutputCertifiedMessages }).Ok.messages.length).toEqual(1);

    // simulate a reconnection
    const res = await wsMessage({
      message: {
        IcWebSocketGatewayStatus: {
          status_index: BigInt(0),
        },
      },
      actor: gateway1,
    });
    expect(res).toMatchObject<CanisterWsMessageResult>({
      Ok: null,
    });

    // check if the canister reset the internal state
    messagesRes = await gateway1.ws_get_messages({
      nonce: BigInt(0),
    });
    expect(messagesRes).toMatchObject<CanisterWsGetMessagesResult>({
      Ok: {
        messages: [],
        cert: expect.any(Uint8Array),
        tree: expect.any(Uint8Array),
      },
    });
  });
});

describe("Canister - ws_get_messages (receive)", () => {
  beforeAll(async () => {
    await assignKeyPairsToClients();

    await wsRegister({
      clientActor: client1,
      clientKey: client1KeyPair.publicKey,
    }, true);

    await wsOpen({
      clientPublicKey: client1KeyPair.publicKey,
      clientSecretKey: client1KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway1,
    }, true);

    // prepare the messages
    for (let i = 0; i < SEND_MESSAGES_COUNT; i++) {
      const appMessage = { text: `test${i}` };
      await wsSend({
        clientPublicKey: client1KeyPair.publicKey,
        actor: client1,
        message: appMessage,
      }, true);
    }

    await commonAgent.fetchRootKey();
  });

  afterAll(async () => {
    await wsWipe(gateway1);
  });

  beforeEach(async () => {
    await sendGatewayStatusMessage();
  });

  it("registered gateway can receive correct amount of messages", async () => {
    for (let i = 0; i < SEND_MESSAGES_COUNT; i++) {
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
      expect(messagesResult.messages.length).toBe(
        SEND_MESSAGES_COUNT - i > MAX_NUMBER_OF_RETURNED_MESSAGES
          ? MAX_NUMBER_OF_RETURNED_MESSAGES
          : SEND_MESSAGES_COUNT - i
      );
    }

    // try to get more messages than available
    const res = await gateway1.ws_get_messages({
      nonce: BigInt(SEND_MESSAGES_COUNT),
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
      nonce: BigInt(0),
    });

    const firstBatchMessagesResult = (firstBatchRes as { Ok: CanisterOutputCertifiedMessages }).Ok;
    for (let i = 0; i < firstBatchMessagesResult.messages.length; i++) {
      const message = firstBatchMessagesResult.messages[i];
      expect(message.client_key).toEqual(client1KeyPair.publicKey);
      const decodedContent = Cbor.decode<WebsocketMessage>(new Uint8Array(message.content));
      expect(decodedContent).toMatchObject<WebsocketMessage>({
        client_key: client1KeyPair.publicKey,
        message: expect.any(Uint8Array),
        sequence_num: i + 1,
        timestamp: expect.any(Object), // weird timestamp deserialization
      });
      expect(IDL.decode([IDL.Record({ 'text': IDL.Text })], decodedContent.message as Uint8Array)).toEqual([{ text: `test${i}` }]);

      // check the certification
      await expect(
        isValidCertificate(
          canisterId,
          firstBatchMessagesResult.cert as Uint8Array,
          firstBatchMessagesResult.tree as Uint8Array,
          commonAgent
        )
      ).resolves.toBe(true);
      await expect(
        isMessageBodyValid(
          message.key,
          message.content as Uint8Array,
          firstBatchMessagesResult.tree as Uint8Array,
        )
      ).resolves.toBe(true);
    }

    // second batch of messages, starting from the last nonce of the first batch
    const secondBatchRes = await gateway1.ws_get_messages({
      nonce: BigInt(MAX_NUMBER_OF_RETURNED_MESSAGES),
    });

    const secondBatchMessagesResult = (secondBatchRes as { Ok: CanisterOutputCertifiedMessages }).Ok;
    for (let i = 0; i < secondBatchMessagesResult.messages.length; i++) {
      const message = secondBatchMessagesResult.messages[i];
      expect(message.client_key).toEqual(client1KeyPair.publicKey);
      const decodedContent = Cbor.decode<WebsocketMessage>(new Uint8Array(message.content));
      expect(decodedContent).toMatchObject<WebsocketMessage>({
        client_key: client1KeyPair.publicKey,
        message: expect.any(Uint8Array),
        sequence_num: i + MAX_NUMBER_OF_RETURNED_MESSAGES + 1,
        timestamp: expect.any(Object), // weird timestamp deserialization
      });
      expect(IDL.decode([IDL.Record({ 'text': IDL.Text })], decodedContent.message as Uint8Array)).toEqual([{ text: `test${i + MAX_NUMBER_OF_RETURNED_MESSAGES}` }]);

      // check the certification
      await expect(
        isValidCertificate(
          canisterId,
          secondBatchMessagesResult.cert as Uint8Array,
          secondBatchMessagesResult.tree as Uint8Array,
          commonAgent
        )
      ).resolves.toBe(true);
      await expect(
        isMessageBodyValid(
          message.key,
          message.content as Uint8Array,
          secondBatchMessagesResult.tree as Uint8Array,
        )
      ).resolves.toBe(true);
    }
  });
});

describe("Canister - ws_close", () => {
  beforeAll(async () => {
    await assignKeyPairsToClients();

    await wsRegister({
      clientActor: client1,
      clientKey: client1KeyPair.publicKey,
    }, true);

    await wsOpen({
      clientPublicKey: client1KeyPair.publicKey,
      clientSecretKey: client1KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway1,
    }, true);
  });

  afterAll(async () => {
    await wsWipe(gateway1);
  });

  beforeEach(async () => {
    await sendGatewayStatusMessage();
  });

  it("fails if gateway is not registered", async () => {
    const res = await wsClose({
      clientPublicKey: client1KeyPair.publicKey,
      gatewayActor: gateway2,
    });

    expect(res).toMatchObject<CanisterWsCloseResult>({
      Err: "caller is not the gateway that has been registered during CDK initialization",
    });
  });

  it("fails if client is not registered", async () => {
    const res = await wsClose({
      clientPublicKey: client2KeyPair.publicKey,
      gatewayActor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsCloseResult>({
      Err: "client's public key has not been previously registered by client",
    });
  });

  it("should close the websocket for a registered client", async () => {
    const res = await wsClose({
      clientPublicKey: client1KeyPair.publicKey,
      gatewayActor: gateway1,
    });

    expect(res).toMatchObject<CanisterWsCloseResult>({
      Ok: null,
    });
  });
});

describe("Canister - ws_send", () => {
  beforeAll(async () => {
    await assignKeyPairsToClients();

    await wsRegister({
      clientActor: client1,
      clientKey: client1KeyPair.publicKey,
    }, true);

    await wsOpen({
      clientPublicKey: client1KeyPair.publicKey,
      clientSecretKey: client1KeyPair.secretKey,
      canisterId,
      gatewayActor: gateway1,
    }, true);
  });

  afterAll(async () => {
    await wsWipe(gateway1);
  });

  it("fails if sending a message to a non registered client", async () => {
    const res = await wsSend({
      clientPublicKey: client2KeyPair.publicKey,
      actor: client1,
      message: { text: "test" },
    });

    expect(res).toMatchObject<CanisterWsSendResult>({
      Err: "client's public key has not been previously registered by client",
    });
  });

  it("should send a message to a registered client", async () => {
    const res = await wsSend({
      clientPublicKey: client1KeyPair.publicKey,
      actor: client1,
      message: { text: "test" },
    });

    expect(res).toMatchObject<CanisterWsSendResult>({
      Ok: null,
    });
  });
});
