import "isomorphic-fetch";
import crypto from "isomorphic-webcrypto";

// for nodejs environment
Object.defineProperty(globalThis, 'crypto', {
  value: {
    getRandomValues: crypto.getRandomValues,
    subtle: crypto.subtle,
  }
});
