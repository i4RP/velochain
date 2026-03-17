/**
 * VeloChain Client SDK
 *
 * TypeScript/JavaScript SDK for connecting to VeloChain game blockchain nodes.
 * Provides RPC communication, WebSocket real-time streaming, wallet management,
 * and player session handling.
 *
 * @example
 * ```ts
 * import { VelochainClient, Wallet } from "@velochain/client-sdk";
 *
 * const client = new VelochainClient({ rpcUrl: "http://localhost:8545" });
 * const wallet = Wallet.generate();
 *
 * // Query game state
 * const world = await client.rpc.getWorldInfo();
 * const players = await client.rpc.getAllPlayers();
 *
 * // Connect and play
 * const session = client.createSession(wallet);
 * await session.connect();
 * await session.move(10, 20, 0);
 * await session.chat("Hello world!");
 * ```
 *
 * @packageDocumentation
 */

export { RpcClient, RpcError } from "./rpc";
export { WsClient, WsState } from "./websocket";
export type { EventHandler, Subscription } from "./websocket";
export { Wallet } from "./wallet";
export { PlayerSession } from "./session";
export type { SessionEvent, SessionEventHandler } from "./session";

// Re-export all types
export type {
  TxType,
  GameAction,
  Transaction,
  Signature,
  SignedTransaction,
  RpcBlock,
  RpcReceipt,
  RpcLog,
  PlayerInfo,
  EntitySnapshot,
  WorldInfo,
  NewBlockEvent,
  GameTickEvent,
  PendingTransactionEvent,
  PlayerStateEvent,
  ChatMessageEvent,
  EntityUpdateEvent,
  GameEvent,
  SessionState,
  SessionInfo,
  VelochainConfig,
} from "./types";

export { SessionState, DEFAULT_CONFIG } from "./types";

import type { VelochainConfig } from "./types";
import { RpcClient } from "./rpc";
import { WsClient } from "./websocket";
import { Wallet } from "./wallet";
import { PlayerSession } from "./session";

/**
 * Main VeloChain client - convenience wrapper combining RPC, WebSocket, and session management.
 *
 * @example
 * ```ts
 * const client = new VelochainClient({
 *   rpcUrl: "http://localhost:8545",
 *   wsUrl: "ws://localhost:8545",
 * });
 *
 * // Use RPC client directly
 * const blockNumber = await client.rpc.blockNumber();
 *
 * // Use WebSocket for real-time updates
 * await client.ws.connect();
 * client.ws.subscribeNewBlocks((block) => console.log("New block:", block));
 *
 * // Create a player session
 * const wallet = Wallet.generate();
 * const session = client.createSession(wallet);
 * await session.connect();
 * ```
 */
export class VelochainClient {
  /** JSON-RPC client for querying chain and game state. */
  readonly rpc: RpcClient;

  /** WebSocket client for real-time event streaming. */
  readonly ws: WsClient;

  /** Client configuration. */
  readonly config: Partial<VelochainConfig>;

  constructor(config: Partial<VelochainConfig> = {}) {
    this.config = config;
    this.rpc = new RpcClient(config);
    this.ws = new WsClient(config);
  }

  /**
   * Create a player session with the given wallet.
   * The session manages connection lifecycle, heartbeats, and game actions.
   */
  createSession(wallet: Wallet): PlayerSession {
    return new PlayerSession(wallet, this.config);
  }

  /**
   * Quick connect: generate a wallet and create a connected session.
   * Useful for testing and quick prototyping.
   */
  async quickConnect(): Promise<{ wallet: Wallet; session: PlayerSession }> {
    const wallet = Wallet.generate();
    const session = this.createSession(wallet);
    await session.connect();
    return { wallet, session };
  }
}
