import { ClientKey } from "../../src/declarations/test_canister/test_canister.did";

export const formatClientKey = (clientKey: ClientKey): string => {
  return `${clientKey.client_principal.toText()}_${clientKey.client_nonce.toString()}`;
};
