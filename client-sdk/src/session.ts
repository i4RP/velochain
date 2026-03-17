/**
 * Player session management for VeloChain.
 *
 * Manages the lifecycle of a player's connection to the game world,
 * including authentication, heartbeats, and state tracking.
 */

import type { PlayerInfo, SessionState, SessionInfo, VelochainConfig } from "./types";
import { SessionState as State } from "./types";
import { RpcClient } from "./rpc";
import { WsClient } from "./websocket";
import { Wallet } from "./wallet";

/** Session event types. */
export type SessionEvent =
  | { type: "connected"; sessionId: string }
  | { type: "disconnected"; reason: string }
  | { type: "playerSpawned"; player: PlayerInfo }
  | { type: "playerUpdated"; player: PlayerInfo }
  | { type: "error"; error: Error }
  | { type: "stateChanged"; from: SessionState; to: SessionState };

/** Session event handler. */
export type SessionEventHandler = (event: SessionEvent) => void;

/**
 * Manages a player's session with the VeloChain game world.
 *
 * Handles:
 * - Authentication via wallet signature
 * - Session lifecycle (connect, disconnect, reconnect)
 * - Heartbeat keep-alive
 * - Player state tracking
 * - Auto-reconnection on disconnect
 */
export class PlayerSession {
  private readonly rpc: RpcClient;
  private readonly ws: WsClient;
  private readonly wallet: Wallet;
  private readonly chainId: number;

  private sessionId: string | null = null;
  private state: SessionState = State.Disconnected;
  private playerInfo: PlayerInfo | null = null;
  private heartbeatInterval: ReturnType<typeof setInterval> | null = null;
  private eventHandlers: SessionEventHandler[] = [];
  private connectedAt = 0;
  private lastActivity = 0;

  constructor(
    wallet: Wallet,
    config: Partial<VelochainConfig> = {}
  ) {
    this.wallet = wallet;
    this.chainId = config.chainId ?? 27181;
    this.rpc = new RpcClient(config);
    this.ws = new WsClient(config);

    // Setup WebSocket event handlers
    this.ws.onOpen(() => {
      this.updateActivity();
    });

    this.ws.onClose((_code, reason) => {
      if (this.state === State.Playing || this.state === State.Connected) {
        this.setState(State.Reconnecting);
        this.emit({ type: "disconnected", reason });
      }
    });

    this.ws.onError(() => {
      this.emit({ type: "error", error: new Error("WebSocket error") });
    });

    this.ws.onReconnect(() => {
      this.setState(State.Reconnecting);
    });
  }

  /** Get current session state. */
  getState(): SessionState {
    return this.state;
  }

  /** Get session info. */
  getSessionInfo(): SessionInfo | null {
    if (!this.sessionId) return null;
    return {
      sessionId: this.sessionId,
      address: this.wallet.getAddress(),
      state: this.state,
      connectedAt: this.connectedAt,
      lastActivity: this.lastActivity,
      playerInfo: this.playerInfo ?? undefined,
    };
  }

  /** Get current player info. */
  getPlayerInfo(): PlayerInfo | null {
    return this.playerInfo;
  }

  /** Register a session event handler. */
  on(handler: SessionEventHandler): () => void {
    this.eventHandlers.push(handler);
    return () => {
      this.eventHandlers = this.eventHandlers.filter((h) => h !== handler);
    };
  }

  /**
   * Connect to the game world.
   * Authenticates via wallet signature and establishes WebSocket connection.
   */
  async connect(): Promise<string> {
    if (this.state === State.Connected || this.state === State.Playing) {
      return this.sessionId!;
    }

    this.setState(State.Connecting);

    try {
      // 1. Sign authentication message
      const address = this.wallet.getAddress();
      const authMessage = `VeloChain Session Auth: ${address} at ${Date.now()}`;
      const signature = this.wallet.signMessage(authMessage);

      // 2. Connect via RPC
      this.sessionId = await this.rpc.connectPlayer(address, signature);
      this.connectedAt = Date.now();
      this.updateActivity();

      // 3. Connect WebSocket for real-time updates
      try {
        await this.ws.connect();
      } catch {
        // WebSocket failure is non-fatal; RPC still works
      }

      // 4. Start heartbeat
      this.startHeartbeat();

      // 5. Fetch initial player state
      await this.refreshPlayerInfo();

      this.setState(State.Connected);
      this.emit({ type: "connected", sessionId: this.sessionId });

      return this.sessionId;
    } catch (error) {
      this.setState(State.Disconnected);
      const err = error instanceof Error ? error : new Error(String(error));
      this.emit({ type: "error", error: err });
      throw err;
    }
  }

  /** Disconnect from the game world. */
  async disconnect(): Promise<void> {
    if (this.state === State.Disconnected) return;

    this.stopHeartbeat();

    try {
      if (this.sessionId) {
        await this.rpc.disconnectPlayer(this.sessionId);
      }
    } catch {
      // Ignore disconnect errors
    }

    this.ws.disconnect();
    this.sessionId = null;
    this.playerInfo = null;
    this.setState(State.Disconnected);
    this.emit({ type: "disconnected", reason: "Client requested disconnect" });
  }

  // ---- Game Actions ----

  /** Move player to position. */
  async move(x: number, y: number, z: number): Promise<string> {
    this.updateActivity();
    return this.rpc.submitAction("move", { x, y, z });
  }

  /** Attack an entity. */
  async attack(targetEntityId: number): Promise<string> {
    this.updateActivity();
    return this.rpc.submitAction("attack", { target_entity_id: targetEntityId });
  }

  /** Send a chat message. */
  async chat(message: string): Promise<string> {
    this.updateActivity();
    return this.rpc.submitAction("chat", { message });
  }

  /** Respawn the player. */
  async respawn(): Promise<string> {
    this.updateActivity();
    return this.rpc.submitAction("respawn", {});
  }

  /**
   * Send a signed game action transaction.
   * Uses the wallet to sign and submit via eth_sendRawTransaction.
   */
  async sendSignedAction(action: import("./types").GameAction): Promise<string> {
    this.updateActivity();
    const nonce = await this.rpc.getTransactionCount(this.wallet.getAddress());
    const rawTx = this.wallet.createGameAction(this.chainId, nonce, action);
    return this.rpc.sendRawTransaction(rawTx);
  }

  // ---- Subscriptions (via WebSocket) ----

  /** Subscribe to game tick updates. */
  onGameTick(handler: (tick: import("./types").GameTickEvent) => void): () => void {
    const sub = this.ws.subscribeGameTicks(handler);
    return sub.unsubscribe;
  }

  /** Subscribe to new block events. */
  onNewBlock(handler: (block: import("./types").NewBlockEvent) => void): () => void {
    const sub = this.ws.subscribeNewBlocks(handler);
    return sub.unsubscribe;
  }

  /** Subscribe to chat messages. */
  onChatMessage(handler: (msg: import("./types").ChatMessageEvent) => void): () => void {
    const sub = this.ws.subscribeChatMessages(handler);
    return sub.unsubscribe;
  }

  /** Subscribe to entity updates in the viewport. */
  onEntityUpdate(handler: (update: import("./types").EntityUpdateEvent) => void): () => void {
    const sub = this.ws.subscribeEntityUpdates(handler);
    return sub.unsubscribe;
  }

  // ---- Internal ----

  private async refreshPlayerInfo(): Promise<void> {
    try {
      const info = await this.rpc.getPlayer(this.wallet.getAddress());
      if (info) {
        const isNew = this.playerInfo === null;
        this.playerInfo = info;
        if (isNew) {
          this.emit({ type: "playerSpawned", player: info });
        } else {
          this.emit({ type: "playerUpdated", player: info });
        }
      }
    } catch {
      // Player might not be spawned yet
    }
  }

  private startHeartbeat(): void {
    this.stopHeartbeat();
    this.heartbeatInterval = setInterval(async () => {
      if (this.sessionId) {
        try {
          await this.rpc.heartbeat(this.sessionId);
          this.updateActivity();
        } catch {
          // Heartbeat failure may indicate connection issues
        }
      }
    }, 10000); // Every 10 seconds
  }

  private stopHeartbeat(): void {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval);
      this.heartbeatInterval = null;
    }
  }

  private setState(newState: SessionState): void {
    if (this.state !== newState) {
      const from = this.state;
      this.state = newState;
      this.emit({ type: "stateChanged", from, to: newState });
    }
  }

  private updateActivity(): void {
    this.lastActivity = Date.now();
  }

  private emit(event: SessionEvent): void {
    for (const handler of this.eventHandlers) {
      try {
        handler(event);
      } catch {
        // Don't let handler errors break the session
      }
    }
  }
}
