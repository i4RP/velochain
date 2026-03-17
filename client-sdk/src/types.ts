/**
 * Core types for VeloChain client SDK.
 * Mirrors the Rust types defined in velochain-primitives and velochain-game-engine.
 */

// ---- Chain Types ----

/** Transaction type identifiers matching Rust TxType enum. */
export enum TxType {
  Legacy = 0,
  AccessList = 1,
  DynamicFee = 2,
  GameAction = 100,
}

/** Game action types matching Rust GameAction enum. */
export type GameAction =
  | { type: "move"; x: number; y: number; z: number }
  | { type: "attack"; target_entity_id: number }
  | { type: "interact"; target_entity_id: number }
  | { type: "placeBlock"; x: number; y: number; z: number; block_type: number }
  | { type: "breakBlock"; x: number; y: number; z: number }
  | { type: "craft"; recipe_id: number }
  | { type: "chat"; message: string }
  | { type: "respawn" };

/** Unsigned transaction data. */
export interface Transaction {
  tx_type: TxType;
  chain_id: number;
  nonce: number;
  gas_price?: string;
  max_fee_per_gas?: string;
  max_priority_fee_per_gas?: string;
  gas_limit: number;
  to?: string;
  value: string;
  input: string;
  game_action?: GameAction;
}

/** ECDSA signature. */
export interface Signature {
  v: number;
  r: string;
  s: string;
}

/** Signed transaction. */
export interface SignedTransaction {
  transaction: Transaction;
  signature: Signature;
  hash: string;
}

// ---- RPC Response Types ----

/** Block information from eth_getBlockByNumber / eth_getBlockByHash. */
export interface RpcBlock {
  number: string;
  hash: string;
  parentHash: string;
  timestamp: string;
  gasLimit: string;
  gasUsed: string;
  beneficiary: string;
  stateRoot: string;
  transactionsRoot: string;
  receiptsRoot: string;
  gameTick: number;
  gameStateRoot: string;
  difficulty: string;
  transactions: string[];
}

/** Transaction receipt from eth_getTransactionReceipt. */
export interface RpcReceipt {
  transactionHash: string;
  blockNumber: string;
  blockHash: string;
  transactionIndex: string;
  success: boolean;
  gasUsed: string;
  cumulativeGasUsed: string;
  contractAddress?: string;
  logs: unknown[];
}

/** Log entry from eth_getLogs. */
export interface RpcLog {
  address: string;
  topics: string[];
  data: string;
  blockNumber: string;
  blockHash: string;
  transactionHash: string;
  transactionIndex: string;
  logIndex: string;
}

// ---- Game Types ----

/** Player information from game_getPlayer / game_getAllPlayers. */
export interface PlayerInfo {
  entity_id: number;
  address: string;
  position: [number, number, number];
  health: number;
  max_health: number;
  level: number;
  is_alive: boolean;
}

/** Entity snapshot from game_getEntitiesInArea. */
export interface EntitySnapshot {
  entity_id: number;
  entity_type: string;
  position: [number, number, number];
  health?: [number, number, number];
}

/** World information from game_getWorldInfo. */
export interface WorldInfo {
  current_tick: number;
  entity_count: number;
  player_count: number;
  seed: number;
}

// ---- WebSocket Event Types ----

/** New block event from WebSocket subscription. */
export interface NewBlockEvent {
  type: "newBlock";
  number: number;
  hash: string;
  tx_count: number;
  timestamp: number;
}

/** Game tick event from WebSocket subscription. */
export interface GameTickEvent {
  type: "gameTick";
  tick: number;
  entity_count: number;
  player_count: number;
  state_root: string;
}

/** Pending transaction event. */
export interface PendingTransactionEvent {
  type: "pendingTransaction";
  hash: string;
}

/** Player state change event (enhanced subscription). */
export interface PlayerStateEvent {
  type: "playerState";
  address: string;
  position: [number, number, number];
  health: number;
  is_alive: boolean;
}

/** Chat message event (enhanced subscription). */
export interface ChatMessageEvent {
  type: "chatMessage";
  sender: string;
  message: string;
  tick: number;
}

/** Entity update event with delta information. */
export interface EntityUpdateEvent {
  type: "entityUpdate";
  entity_id: number;
  entity_type: string;
  position: [number, number, number];
  health?: number;
  removed: boolean;
}

/** All possible game events. */
export type GameEvent =
  | NewBlockEvent
  | GameTickEvent
  | PendingTransactionEvent
  | PlayerStateEvent
  | ChatMessageEvent
  | EntityUpdateEvent;

// ---- Session Types ----

/** Player session state. */
export enum SessionState {
  Disconnected = "disconnected",
  Connecting = "connecting",
  Connected = "connected",
  Playing = "playing",
  Reconnecting = "reconnecting",
}

/** Session information. */
export interface SessionInfo {
  sessionId: string;
  address: string;
  state: SessionState;
  connectedAt: number;
  lastActivity: number;
  playerInfo?: PlayerInfo;
}

// ---- Configuration ----

/** SDK configuration options. */
export interface VelochainConfig {
  /** RPC endpoint URL (HTTP). */
  rpcUrl: string;
  /** WebSocket endpoint URL. */
  wsUrl?: string;
  /** Chain ID (default: 27181). */
  chainId?: number;
  /** Request timeout in ms (default: 10000). */
  timeout?: number;
  /** Auto-reconnect WebSocket on disconnect (default: true). */
  autoReconnect?: boolean;
  /** Max reconnection attempts (default: 5). */
  maxReconnectAttempts?: number;
  /** Reconnection delay in ms (default: 1000). */
  reconnectDelay?: number;
}

/** Default configuration values. */
export const DEFAULT_CONFIG: Required<VelochainConfig> = {
  rpcUrl: "http://localhost:8545",
  wsUrl: "ws://localhost:8545",
  chainId: 27181,
  timeout: 10000,
  autoReconnect: true,
  maxReconnectAttempts: 5,
  reconnectDelay: 1000,
};
