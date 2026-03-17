/**
 * WebSocket client for real-time VeloChain game state streaming.
 *
 * Manages WebSocket connections with automatic reconnection,
 * subscription management, and event dispatching.
 */

import type {
  GameEvent,
  VelochainConfig,
  NewBlockEvent,
  GameTickEvent,
  PendingTransactionEvent,
  PlayerStateEvent,
  ChatMessageEvent,
  EntityUpdateEvent,
} from "./types";
import { DEFAULT_CONFIG } from "./types";

/** Event handler callback type. */
export type EventHandler<T = GameEvent> = (event: T) => void;

/** Subscription handle for unsubscribing. */
export interface Subscription {
  id: string;
  unsubscribe: () => void;
}

/** WebSocket connection state. */
export enum WsState {
  Connecting = "connecting",
  Open = "open",
  Closing = "closing",
  Closed = "closed",
  Reconnecting = "reconnecting",
}

/**
 * WebSocket client for real-time game state updates.
 *
 * Supports VeloChain-specific subscriptions:
 * - New blocks
 * - Game ticks with entity counts
 * - Pending transactions
 * - Player state changes (enhanced)
 * - Chat messages (enhanced)
 * - Entity updates with diffs (enhanced)
 */
export class WsClient {
  private ws: WebSocket | null = null;
  private readonly wsUrl: string;
  private readonly autoReconnect: boolean;
  private readonly maxReconnectAttempts: number;
  private readonly reconnectDelay: number;
  private reconnectAttempts = 0;
  private requestId = 0;
  private state: WsState = WsState.Closed;

  // Subscription management
  private subscriptions = new Map<string, EventHandler>();
  private pendingRequests = new Map<number, {
    resolve: (value: unknown) => void;
    reject: (reason: Error) => void;
  }>();

  // Connection event handlers
  private onOpenHandlers: (() => void)[] = [];
  private onCloseHandlers: ((code: number, reason: string) => void)[] = [];
  private onErrorHandlers: ((error: Event) => void)[] = [];
  private onReconnectHandlers: ((attempt: number) => void)[] = [];

  constructor(config: Partial<VelochainConfig> = {}) {
    const merged = { ...DEFAULT_CONFIG, ...config };
    this.wsUrl = merged.wsUrl;
    this.autoReconnect = merged.autoReconnect;
    this.maxReconnectAttempts = merged.maxReconnectAttempts;
    this.reconnectDelay = merged.reconnectDelay;
  }

  /** Get current connection state. */
  getState(): WsState {
    return this.state;
  }

  /** Connect to the WebSocket server. */
  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this.state === WsState.Open) {
        resolve();
        return;
      }

      this.state = WsState.Connecting;

      try {
        this.ws = new WebSocket(this.wsUrl);
      } catch (err) {
        this.state = WsState.Closed;
        reject(err);
        return;
      }

      this.ws.onopen = () => {
        this.state = WsState.Open;
        this.reconnectAttempts = 0;
        this.onOpenHandlers.forEach((h) => h());

        // Re-subscribe after reconnection
        this.resubscribeAll();
        resolve();
      };

      this.ws.onclose = (event) => {
        this.state = WsState.Closed;
        this.onCloseHandlers.forEach((h) => h(event.code, event.reason));

        // Reject pending requests
        this.pendingRequests.forEach(({ reject: rej }) => {
          rej(new Error("WebSocket closed"));
        });
        this.pendingRequests.clear();

        // Auto-reconnect
        if (this.autoReconnect && this.reconnectAttempts < this.maxReconnectAttempts) {
          this.scheduleReconnect();
        }
      };

      this.ws.onerror = (event) => {
        this.onErrorHandlers.forEach((h) => h(event));
        if (this.state === WsState.Connecting) {
          reject(new Error("WebSocket connection failed"));
        }
      };

      this.ws.onmessage = (event) => {
        this.handleMessage(event.data as string);
      };
    });
  }

  /** Disconnect from the WebSocket server. */
  disconnect(): void {
    this.autoReconnect && (this.reconnectAttempts = this.maxReconnectAttempts); // Prevent reconnect
    if (this.ws) {
      this.state = WsState.Closing;
      this.ws.close(1000, "Client disconnect");
      this.ws = null;
    }
    this.subscriptions.clear();
    this.pendingRequests.clear();
    this.state = WsState.Closed;
  }

  // ---- Subscription methods ----

  /** Subscribe to new block events. */
  subscribeNewBlocks(handler: EventHandler<NewBlockEvent>): Subscription {
    return this.subscribe("velochain_subscribeNewBlocks", "newBlock", handler as EventHandler);
  }

  /** Subscribe to game tick events. */
  subscribeGameTicks(handler: EventHandler<GameTickEvent>): Subscription {
    return this.subscribe("velochain_subscribeGameTicks", "gameTick", handler as EventHandler);
  }

  /** Subscribe to pending transaction events. */
  subscribePendingTxs(handler: EventHandler<PendingTransactionEvent>): Subscription {
    return this.subscribe(
      "velochain_subscribePendingTxs",
      "pendingTx",
      handler as EventHandler
    );
  }

  /** Subscribe to player state change events (enhanced). */
  subscribePlayerState(handler: EventHandler<PlayerStateEvent>): Subscription {
    return this.subscribe(
      "velochain_subscribePlayerState",
      "playerState",
      handler as EventHandler
    );
  }

  /** Subscribe to chat message events (enhanced). */
  subscribeChatMessages(handler: EventHandler<ChatMessageEvent>): Subscription {
    return this.subscribe(
      "velochain_subscribeChatMessages",
      "chatMessage",
      handler as EventHandler
    );
  }

  /** Subscribe to entity update events (enhanced). */
  subscribeEntityUpdates(handler: EventHandler<EntityUpdateEvent>): Subscription {
    return this.subscribe(
      "velochain_subscribeEntityUpdates",
      "entityUpdate",
      handler as EventHandler
    );
  }

  /** Subscribe using Ethereum-standard eth_subscribe. */
  subscribeEth(
    kind: "newHeads" | "newPendingTransactions",
    handler: EventHandler
  ): Subscription {
    return this.subscribe("eth_subscribe", kind, handler);
  }

  // ---- Connection event handlers ----

  onOpen(handler: () => void): void {
    this.onOpenHandlers.push(handler);
  }

  onClose(handler: (code: number, reason: string) => void): void {
    this.onCloseHandlers.push(handler);
  }

  onError(handler: (error: Event) => void): void {
    this.onErrorHandlers.push(handler);
  }

  onReconnect(handler: (attempt: number) => void): void {
    this.onReconnectHandlers.push(handler);
  }

  // ---- Internal methods ----

  private subscribe(method: string, subKey: string, handler: EventHandler): Subscription {
    const id = `${subKey}_${++this.requestId}`;

    this.subscriptions.set(id, handler);

    // Send subscription request if connected
    if (this.ws && this.state === WsState.Open) {
      this.sendSubscriptionRequest(method, subKey, id);
    }

    return {
      id,
      unsubscribe: () => {
        this.subscriptions.delete(id);
      },
    };
  }

  private sendSubscriptionRequest(method: string, params: string, _subId: string): void {
    if (!this.ws || this.state !== WsState.Open) return;

    const id = ++this.requestId;
    const request = {
      jsonrpc: "2.0",
      id,
      method,
      params: method === "eth_subscribe" ? [params] : [],
    };

    this.ws.send(JSON.stringify(request));
  }

  private handleMessage(data: string): void {
    try {
      const msg = JSON.parse(data);

      // Handle RPC response
      if (msg.id && this.pendingRequests.has(msg.id)) {
        const { resolve, reject } = this.pendingRequests.get(msg.id)!;
        this.pendingRequests.delete(msg.id);
        if (msg.error) {
          reject(new Error(msg.error.message));
        } else {
          resolve(msg.result);
        }
        return;
      }

      // Handle subscription notification
      if (msg.method && msg.params) {
        const event = msg.params.result || msg.params;
        this.dispatchEvent(event);
      }
    } catch {
      // Ignore parse errors for non-JSON messages
    }
  }

  private dispatchEvent(event: GameEvent): void {
    if (!event || !event.type) return;

    for (const [subId, handler] of this.subscriptions) {
      // Match subscription to event type
      if (subId.startsWith(event.type) || subId.startsWith("newBlock") && event.type === "newBlock") {
        try {
          handler(event);
        } catch {
          // Don't let handler errors break the event loop
        }
      }
    }
  }

  private resubscribeAll(): void {
    // Re-create subscriptions after reconnect
    for (const [subId] of this.subscriptions) {
      const eventType = subId.split("_")[0];
      const methodMap: Record<string, string> = {
        newBlock: "velochain_subscribeNewBlocks",
        gameTick: "velochain_subscribeGameTicks",
        pendingTx: "velochain_subscribePendingTxs",
        playerState: "velochain_subscribePlayerState",
        chatMessage: "velochain_subscribeChatMessages",
        entityUpdate: "velochain_subscribeEntityUpdates",
      };
      const method = methodMap[eventType];
      if (method) {
        this.sendSubscriptionRequest(method, eventType, subId);
      }
    }
  }

  private scheduleReconnect(): void {
    this.reconnectAttempts++;
    this.state = WsState.Reconnecting;
    const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);

    this.onReconnectHandlers.forEach((h) => h(this.reconnectAttempts));

    setTimeout(() => {
      if (this.state === WsState.Reconnecting) {
        this.connect().catch(() => {
          // Reconnect failed, will try again if attempts remain
        });
      }
    }, delay);
  }
}
