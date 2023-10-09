import { ClientKey, ClientPrincipal } from "../../src/declarations/test_canister/test_canister.did";
import { generateRandomIdentity } from "./identity";

export const getRandomClientNonce = (): bigint => {
  const array = new BigUint64Array(1);
  globalThis.crypto.getRandomValues(array);
  return array[0];
};

export const generateClientKey = (clientPrincipal: ClientPrincipal): ClientKey => {
  return {
    client_principal: clientPrincipal,
    client_nonce: getRandomClientNonce(),
  };
};

export const getRandomPrincipal = async (): Promise<ClientPrincipal> => {
  const identity = await generateRandomIdentity();
  return identity.getPrincipal();
};
