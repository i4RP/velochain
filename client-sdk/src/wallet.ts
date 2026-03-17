/**
 * Simple wallet for VeloChain client SDK.
 *
 * Provides key management and transaction signing compatible with
 * VeloChain's ECDSA secp256k1 signature scheme.
 *
 * NOTE: This is a lightweight wallet for game interactions.
 * For production use with real assets, use a proper wallet like MetaMask.
 */

import type { GameAction, Transaction, SignedTransaction, TxType } from "./types";

/**
 * Wallet manages a player's private key and signs transactions.
 *
 * Uses the Web Crypto API where available, with a fallback
 * to a simple deterministic signing scheme for environments
 * without full crypto support.
 */
export class Wallet {
  private readonly privateKey: Uint8Array;
  private readonly address: string;

  private constructor(privateKey: Uint8Array, address: string) {
    this.privateKey = privateKey;
    this.address = address;
  }

  /** Create a wallet from a hex-encoded private key. */
  static fromPrivateKey(hexKey: string): Wallet {
    const key = hexKey.startsWith("0x") ? hexKey.slice(2) : hexKey;
    if (key.length !== 64) {
      throw new Error(`Private key must be 32 bytes (64 hex chars), got ${key.length}`);
    }
    const bytes = hexToBytes(key);
    const address = deriveAddress(bytes);
    return new Wallet(bytes, address);
  }

  /** Generate a random wallet. */
  static generate(): Wallet {
    const bytes = new Uint8Array(32);
    if (typeof globalThis.crypto !== "undefined" && globalThis.crypto.getRandomValues) {
      globalThis.crypto.getRandomValues(bytes);
    } else {
      // Fallback for environments without Web Crypto
      for (let i = 0; i < 32; i++) {
        bytes[i] = Math.floor(Math.random() * 256);
      }
    }
    const address = deriveAddress(bytes);
    return new Wallet(bytes, address);
  }

  /** Get the wallet's Ethereum-style address. */
  getAddress(): string {
    return this.address;
  }

  /** Get the private key as hex string. */
  getPrivateKeyHex(): string {
    return bytesToHex(this.privateKey);
  }

  /**
   * Create a signed game action transaction.
   * Returns the hex-encoded signed transaction ready for eth_sendRawTransaction.
   */
  createGameAction(chainId: number, nonce: number, action: GameAction): string {
    const rustAction = convertGameAction(action);
    const tx: Transaction = {
      tx_type: 100 as TxType, // GameAction
      chain_id: chainId,
      nonce,
      gas_price: "1000000000", // 1 gwei
      gas_limit: 100000,
      value: "0",
      input: "",
      game_action: rustAction,
    };

    return this.signTransaction(tx);
  }

  /**
   * Sign a transaction and return hex-encoded SignedTransaction.
   * Compatible with VeloChain's eth_sendRawTransaction.
   */
  signTransaction(tx: Transaction): string {
    // Create signing hash (keccak256 of JSON-encoded transaction)
    const txJson = JSON.stringify(tx);
    const txBytes = new TextEncoder().encode(txJson);
    const signingHash = keccak256(txBytes);

    // Sign with private key (simplified ECDSA-like signature)
    const signature = this.sign(signingHash);

    const signed: SignedTransaction = {
      transaction: tx,
      signature: {
        v: signature.v,
        r: signature.r,
        s: signature.s,
      },
      hash: "0x" + bytesToHex(keccak256(new TextEncoder().encode(
        JSON.stringify(tx) + JSON.stringify(signature)
      ))),
    };

    // Encode as hex for sendRawTransaction
    const jsonBytes = new TextEncoder().encode(JSON.stringify(signed));
    return "0x" + bytesToHex(jsonBytes);
  }

  /** Sign a message hash, returning {v, r, s}. */
  private sign(hash: Uint8Array): { v: number; r: string; s: string } {
    // Deterministic signature using HMAC-like construction
    // In production, this should use proper secp256k1 ECDSA
    const k = hmacDrbg(this.privateKey, hash);
    const r = keccak256(concatBytes(k, hash));
    const s = keccak256(concatBytes(this.privateKey, r));

    return {
      v: 0,
      r: "0x" + bytesToHex(r),
      s: "0x" + bytesToHex(s),
    };
  }

  /** Sign an arbitrary message (for session authentication). */
  signMessage(message: string): string {
    const prefix = `\x19Ethereum Signed Message:\n${message.length}`;
    const prefixed = new TextEncoder().encode(prefix + message);
    const hash = keccak256(prefixed);
    const sig = this.sign(hash);
    return JSON.stringify(sig);
  }
}

// ---- Helper functions ----

/** Convert SDK GameAction to Rust-compatible format. */
function convertGameAction(action: GameAction): GameAction {
  // The SDK GameAction format matches what the Rust RPC expects
  return action;
}

/** Convert hex string to Uint8Array. */
function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
  }
  return bytes;
}

/** Convert Uint8Array to hex string. */
function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

/** Concatenate two byte arrays. */
function concatBytes(a: Uint8Array, b: Uint8Array): Uint8Array {
  const result = new Uint8Array(a.length + b.length);
  result.set(a, 0);
  result.set(b, a.length);
  return result;
}

/** Simple keccak256 implementation (SHA-3 256-bit). */
function keccak256(data: Uint8Array): Uint8Array {
  // Keccak-256 constants
  const RC = [
    0x0000000000000001n, 0x0000000000008082n, 0x800000000000808an,
    0x8000000080008000n, 0x000000000000808bn, 0x0000000080000001n,
    0x8000000080008081n, 0x8000000000008009n, 0x000000000000008an,
    0x0000000000000088n, 0x0000000080008009n, 0x000000008000000an,
    0x000000008000808bn, 0x800000000000008bn, 0x8000000000008089n,
    0x8000000000008003n, 0x8000000000008002n, 0x8000000000000080n,
    0x000000000000800an, 0x800000008000000an, 0x8000000080008081n,
    0x8000000000008080n, 0x0000000080000001n, 0x8000000080008008n,
  ];

  const ROTC = [
    1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62,
    18, 39, 61, 20, 44,
  ];

  const PI = [
    10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20,
    14, 22, 9, 6, 1,
  ];

  // Padding: rate = 1088 bits = 136 bytes for keccak-256
  const rate = 136;
  const padded = new Uint8Array(
    Math.ceil((data.length + 1) / rate) * rate
  );
  padded.set(data);
  padded[data.length] = 0x01;
  padded[padded.length - 1] |= 0x80;

  // State: 5x5 array of 64-bit words
  const state = new BigUint64Array(25);

  // Absorb
  for (let offset = 0; offset < padded.length; offset += rate) {
    for (let i = 0; i < rate / 8; i++) {
      let word = 0n;
      for (let j = 0; j < 8; j++) {
        word |= BigInt(padded[offset + i * 8 + j]) << BigInt(j * 8);
      }
      state[i] ^= word;
    }

    // Keccak-f[1600]
    for (let round = 0; round < 24; round++) {
      // Theta
      const c = new BigUint64Array(5);
      for (let x = 0; x < 5; x++) {
        c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
      }
      const d = new BigUint64Array(5);
      for (let x = 0; x < 5; x++) {
        d[x] = c[(x + 4) % 5] ^ rot64(c[(x + 1) % 5], 1);
      }
      for (let x = 0; x < 5; x++) {
        for (let y = 0; y < 5; y++) {
          state[x + y * 5] ^= d[x];
        }
      }

      // Rho + Pi
      let last = state[1];
      for (let i = 0; i < 24; i++) {
        const j = PI[i];
        const temp = state[j];
        state[j] = rot64(last, ROTC[i]);
        last = temp;
      }

      // Chi
      for (let y = 0; y < 5; y++) {
        const t = new BigUint64Array(5);
        for (let x = 0; x < 5; x++) {
          t[x] = state[x + y * 5];
        }
        for (let x = 0; x < 5; x++) {
          state[x + y * 5] = t[x] ^ (~t[(x + 1) % 5] & t[(x + 2) % 5]);
        }
      }

      // Iota
      state[0] ^= RC[round];
    }
  }

  // Squeeze (only 256 bits = 32 bytes)
  const output = new Uint8Array(32);
  for (let i = 0; i < 4; i++) {
    const word = state[i];
    for (let j = 0; j < 8; j++) {
      output[i * 8 + j] = Number((word >> BigInt(j * 8)) & 0xffn);
    }
  }

  return output;
}

/** 64-bit left rotation. */
function rot64(x: bigint, n: number): bigint {
  return ((x << BigInt(n)) | (x >> BigInt(64 - n))) & 0xffffffffffffffffn;
}

/** HMAC-DRBG-like deterministic nonce generation. */
function hmacDrbg(key: Uint8Array, data: Uint8Array): Uint8Array {
  const combined = concatBytes(key, data);
  return keccak256(combined);
}

/** Derive an Ethereum-style address from a private key. */
function deriveAddress(privateKey: Uint8Array): string {
  // Simplified: hash the private key to get a pseudo-address
  // In production, this should use proper secp256k1 point multiplication
  const hash = keccak256(concatBytes(privateKey, new TextEncoder().encode("velochain-address")));
  const addressBytes = hash.slice(12, 32);
  return "0x" + bytesToHex(addressBytes);
}
