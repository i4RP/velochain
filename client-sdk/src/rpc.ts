/**
 * JSON-RPC client for VeloChain node.
 * Supports both Ethereum-compatible and game-specific RPC methods.
 */

import type {
  RpcBlock,
  RpcReceipt,
  RpcLog,
  PlayerInfo,
  EntitySnapshot,
  WorldInfo,
  VelochainConfig,
} from "./types";
import { DEFAULT_CONFIG } from "./types";

/** JSON-RPC request structure. */
interface JsonRpcRequest {
  jsonrpc: "2.0";
  id: number;
  method: string;
  params: unknown[];
}

/** JSON-RPC response structure. */
interface JsonRpcResponse<T = unknown> {
  jsonrpc: "2.0";
  id: number;
  result?: T;
  error?: { code: number; message: string; data?: unknown };
}

/** RPC client error. */
export class RpcError extends Error {
  constructor(
    public code: number,
    message: string,
    public data?: unknown
  ) {
    super(message);
    this.name = "RpcError";
  }
}

/**
 * JSON-RPC client for communicating with VeloChain nodes.
 *
 * Provides typed methods for all Ethereum-compatible and game-specific
 * RPC endpoints exposed by the VeloChain node.
 */
export class RpcClient {
  private readonly rpcUrl: string;
  private readonly timeout: number;
  private requestId = 0;

  constructor(config: Partial<VelochainConfig> = {}) {
    const merged = { ...DEFAULT_CONFIG, ...config };
    this.rpcUrl = merged.rpcUrl;
    this.timeout = merged.timeout;
  }

  // ---- Low-level RPC ----

  /** Send a raw JSON-RPC request. */
  async request<T>(method: string, params: unknown[] = []): Promise<T> {
    const id = ++this.requestId;
    const body: JsonRpcRequest = { jsonrpc: "2.0", id, method, params };

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeout);

    try {
      const response = await fetch(this.rpcUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
        signal: controller.signal,
      });

      if (!response.ok) {
        throw new RpcError(-32000, `HTTP ${response.status}: ${response.statusText}`);
      }

      const json: JsonRpcResponse<T> = await response.json();

      if (json.error) {
        throw new RpcError(json.error.code, json.error.message, json.error.data);
      }

      return json.result as T;
    } finally {
      clearTimeout(timer);
    }
  }

  // ---- Ethereum-compatible methods ----

  /** Get the chain ID (eth_chainId). */
  async chainId(): Promise<number> {
    const hex = await this.request<string>("eth_chainId");
    return parseInt(hex, 16);
  }

  /** Get the latest block number (eth_blockNumber). */
  async blockNumber(): Promise<number> {
    const hex = await this.request<string>("eth_blockNumber");
    return parseInt(hex, 16);
  }

  /** Get account balance in wei (eth_getBalance). */
  async getBalance(address: string, block?: string): Promise<bigint> {
    const hex = await this.request<string>("eth_getBalance", [address, block ?? "latest"]);
    return BigInt(hex);
  }

  /** Get account nonce (eth_getTransactionCount). */
  async getTransactionCount(address: string, block?: string): Promise<number> {
    const hex = await this.request<string>("eth_getTransactionCount", [
      address,
      block ?? "latest",
    ]);
    return parseInt(hex, 16);
  }

  /** Send a signed raw transaction (eth_sendRawTransaction). */
  async sendRawTransaction(data: string): Promise<string> {
    return this.request<string>("eth_sendRawTransaction", [data]);
  }

  /** Get the current gas price in wei (eth_gasPrice). */
  async gasPrice(): Promise<bigint> {
    const hex = await this.request<string>("eth_gasPrice");
    return BigInt(hex);
  }

  /** Get network version (net_version). */
  async netVersion(): Promise<string> {
    return this.request<string>("net_version");
  }

  /** Get block by number (eth_getBlockByNumber). */
  async getBlockByNumber(
    number: number | "latest",
    fullTxs = false
  ): Promise<RpcBlock | null> {
    const blockParam = number === "latest" ? "latest" : `0x${number.toString(16)}`;
    return this.request<RpcBlock | null>("eth_getBlockByNumber", [blockParam, fullTxs]);
  }

  /** Get block by hash (eth_getBlockByHash). */
  async getBlockByHash(hash: string, fullTxs = false): Promise<RpcBlock | null> {
    return this.request<RpcBlock | null>("eth_getBlockByHash", [hash, fullTxs]);
  }

  /** Get transaction receipt (eth_getTransactionReceipt). */
  async getTransactionReceipt(hash: string): Promise<RpcReceipt | null> {
    return this.request<RpcReceipt | null>("eth_getTransactionReceipt", [hash]);
  }

  /** Execute a read-only call (eth_call). */
  async call(
    tx: { from?: string; to?: string; value?: string; data?: string; gas?: string },
    block?: string
  ): Promise<string> {
    return this.request<string>("eth_call", [tx, block ?? "latest"]);
  }

  /** Estimate gas for a transaction (eth_estimateGas). */
  async estimateGas(tx: {
    from?: string;
    to?: string;
    value?: string;
    data?: string;
  }): Promise<number> {
    const hex = await this.request<string>("eth_estimateGas", [tx]);
    return parseInt(hex, 16);
  }

  /** Get contract code at address (eth_getCode). */
  async getCode(address: string, block?: string): Promise<string> {
    return this.request<string>("eth_getCode", [address, block ?? "latest"]);
  }

  /** Get logs matching a filter (eth_getLogs). */
  async getLogs(filter: {
    fromBlock?: string;
    toBlock?: string;
    address?: string;
    topics?: (string | null)[];
  }): Promise<RpcLog[]> {
    return this.request<RpcLog[]>("eth_getLogs", [filter]);
  }

  // ---- Game-specific methods ----

  /** Get world information (game_getWorldInfo). */
  async getWorldInfo(): Promise<WorldInfo> {
    return this.request<WorldInfo>("game_getWorldInfo");
  }

  /** Get player info by address (game_getPlayer). */
  async getPlayer(address: string): Promise<PlayerInfo | null> {
    return this.request<PlayerInfo | null>("game_getPlayer", [address]);
  }

  /** Get current game tick (game_getCurrentTick). */
  async getCurrentTick(): Promise<number> {
    return this.request<number>("game_getCurrentTick");
  }

  /** Get entity count (game_getEntityCount). */
  async getEntityCount(): Promise<number> {
    return this.request<number>("game_getEntityCount");
  }

  /** Submit a game action (game_submitAction). */
  async submitAction(actionType: string, params: Record<string, unknown>): Promise<string> {
    return this.request<string>("game_submitAction", [actionType, params]);
  }

  /** Get all players in the world (game_getAllPlayers). */
  async getAllPlayers(): Promise<PlayerInfo[]> {
    return this.request<PlayerInfo[]>("game_getAllPlayers");
  }

  /** Get entities in a bounding box area (game_getEntitiesInArea). */
  async getEntitiesInArea(
    minX: number,
    minY: number,
    maxX: number,
    maxY: number
  ): Promise<EntitySnapshot[]> {
    return this.request<EntitySnapshot[]>("game_getEntitiesInArea", [minX, minY, maxX, maxY]);
  }

  // ---- Session methods (Phase 9.3) ----

  /** Connect player to the game session (game_connectPlayer). */
  async connectPlayer(address: string, signature: string): Promise<string> {
    return this.request<string>("game_connectPlayer", [address, signature]);
  }

  /** Disconnect player session (game_disconnectPlayer). */
  async disconnectPlayer(sessionId: string): Promise<boolean> {
    return this.request<boolean>("game_disconnectPlayer", [sessionId]);
  }

  /** Get active sessions (game_getActiveSessions). */
  async getActiveSessions(): Promise<number> {
    return this.request<number>("game_getActiveSessions");
  }

  /** Heartbeat to keep session alive (game_heartbeat). */
  async heartbeat(sessionId: string): Promise<boolean> {
    return this.request<boolean>("game_heartbeat", [sessionId]);
  }

  // ---- Admin methods ----

  /** Get node info (admin_nodeInfo). */
  async nodeInfo(): Promise<Record<string, unknown>> {
    return this.request<Record<string, unknown>>("admin_nodeInfo");
  }

  /** Health check (admin_health). */
  async health(): Promise<Record<string, unknown>> {
    return this.request<Record<string, unknown>>("admin_health");
  }

  /** Get peer count (admin_peerCount). */
  async peerCount(): Promise<number> {
    return this.request<number>("admin_peerCount");
  }
}
