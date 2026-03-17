# VeloChain Game Client Protocol Specification

Version: 1.0.0

## Overview

This document defines the communication protocol between VeloChain game clients and nodes. The protocol operates over two transport layers:

- **HTTP JSON-RPC**: Request-response queries and transaction submission
- **WebSocket JSON-RPC**: Real-time event subscriptions and streaming

## Transport

### HTTP (JSON-RPC 2.0)

- Default endpoint: `http://localhost:8545`
- Content-Type: `application/json`
- All methods follow JSON-RPC 2.0 specification

### WebSocket (JSON-RPC 2.0)

- Default endpoint: `ws://localhost:8545`
- Supports subscription-based streaming
- Auto-reconnection recommended with exponential backoff

## Authentication

### Session-Based Authentication

1. Client generates or loads a wallet (secp256k1 keypair)
2. Client signs an auth message: `VeloChain Session Auth: {address} at {timestamp}`
3. Client calls `game_connectPlayer(address, signature)` to obtain a session ID
4. Session ID is used for subsequent heartbeats and disconnect
5. Heartbeat interval: 10 seconds via `game_heartbeat(sessionId)`
6. Session timeout: 300 seconds (5 minutes) without heartbeat

## Message Format

### Request

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "game_getWorldInfo",
  "params": []
}
```

### Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "current_tick": 12345,
    "entity_count": 42,
    "player_count": 3,
    "seed": 27181
  }
}
```

### Error Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32000,
    "message": "Internal error description"
  }
}
```

### Subscription Notification

```json
{
  "jsonrpc": "2.0",
  "method": "velochain_subscription",
  "params": {
    "subscription": "0x1",
    "result": {
      "type": "gameTick",
      "tick": 12345,
      "entity_count": 42,
      "player_count": 3,
      "state_root": "0xabc..."
    }
  }
}
```

## RPC Methods

### Ethereum-Compatible Methods

| Method | Params | Returns | Description |
|--------|--------|---------|-------------|
| `eth_chainId` | none | hex string | Chain ID |
| `eth_blockNumber` | none | hex string | Latest block number |
| `eth_getBalance` | address, block | hex string | Account balance (wei) |
| `eth_getTransactionCount` | address, block | hex string | Account nonce |
| `eth_sendRawTransaction` | hex data | tx hash | Submit signed transaction |
| `eth_gasPrice` | none | hex string | Current gas price |
| `eth_getBlockByNumber` | number, fullTxs | RpcBlock | Block by number |
| `eth_getBlockByHash` | hash, fullTxs | RpcBlock | Block by hash |
| `eth_getTransactionReceipt` | hash | RpcReceipt | Transaction receipt |
| `eth_call` | CallRequest, block | hex string | Read-only call |
| `eth_estimateGas` | CallRequest, block | hex string | Gas estimation |
| `eth_getCode` | address, block | hex string | Contract code |
| `eth_getLogs` | LogFilter | RpcLog[] | Event logs |
| `net_version` | none | string | Network version |

### Game-Specific Methods

| Method | Params | Returns | Description |
|--------|--------|---------|-------------|
| `game_getWorldInfo` | none | WorldInfo | World state summary |
| `game_getPlayer` | address | PlayerInfo? | Player data by address |
| `game_getCurrentTick` | none | number | Current game tick |
| `game_getEntityCount` | none | number | Total entity count |
| `game_submitAction` | type, params | tx hash | Submit game action |
| `game_getAllPlayers` | none | PlayerInfo[] | All player data |
| `game_getEntitiesInArea` | minX, minY, maxX, maxY | EntitySnapshot[] | Area query |

### Session Methods

| Method | Params | Returns | Description |
|--------|--------|---------|-------------|
| `game_connectPlayer` | address, signature | session ID | Connect player |
| `game_disconnectPlayer` | sessionId | boolean | Disconnect player |
| `game_heartbeat` | sessionId | boolean | Keep session alive |
| `game_getActiveSessions` | none | number | Active session count |
| `game_getSessionInfo` | sessionId | SessionInfo? | Session details |

### Admin Methods

| Method | Params | Returns | Description |
|--------|--------|---------|-------------|
| `admin_nodeInfo` | none | NodeInfo | Node details |
| `admin_health` | none | HealthStatus | Health check |
| `admin_peerCount` | none | number | Connected peers |

## WebSocket Subscriptions

### VeloChain Subscriptions

| Method | Event Type | Description |
|--------|-----------|-------------|
| `velochain_subscribeNewBlocks` | `newBlock` | New block produced |
| `velochain_subscribeGameTicks` | `gameTick` | Game tick completed |
| `velochain_subscribePendingTxs` | `pendingTx` | New pending transaction |
| `velochain_subscribePlayerState` | `playerState` | Player state change |
| `velochain_subscribeChatMessages` | `chatMessage` | Chat message broadcast |
| `velochain_subscribeEntityUpdates` | `entityUpdate` | Entity spawn/move/despawn |

### Ethereum Standard Subscriptions

| Method | Kind | Description |
|--------|------|-------------|
| `eth_subscribe` | `newHeads` | New block headers |
| `eth_subscribe` | `newPendingTransactions` | Pending transactions |

## Event Schemas

### NewBlock

```json
{
  "type": "newBlock",
  "number": 100,
  "hash": "0xabc...",
  "tx_count": 5,
  "timestamp": 1700000000
}
```

### GameTick

```json
{
  "type": "gameTick",
  "tick": 500,
  "entity_count": 42,
  "player_count": 3,
  "state_root": "0xdef..."
}
```

### PlayerState

```json
{
  "type": "playerState",
  "address": "0x1234...",
  "position": [10.0, 20.0, 0.0],
  "health": 85.0,
  "is_alive": true
}
```

### ChatMessage

```json
{
  "type": "chatMessage",
  "sender": "0x1234...",
  "message": "Hello world!",
  "tick": 500
}
```

### EntityUpdate

```json
{
  "type": "entityUpdate",
  "entity_id": 42,
  "entity_type": "npc:merchant",
  "position": [10.0, 10.0, 0.0],
  "health": 100.0,
  "removed": false
}
```

## Game Action Types

Actions are submitted via `game_submitAction(type, params)`:

| Action | Params | Description |
|--------|--------|-------------|
| `move` | `{x, y, z}` | Move player to position |
| `attack` | `{target_entity_id}` | Attack an entity |
| `chat` | `{message}` | Send chat message |
| `respawn` | `{}` | Respawn dead player |

## Data Types

### WorldInfo

```typescript
{
  current_tick: number;
  entity_count: number;
  player_count: number;
  seed: number;
}
```

### PlayerInfo

```typescript
{
  entity_id: number;
  address: string;
  position: [number, number, number];
  health: number;
  max_health: number;
  level: number;
  is_alive: boolean;
}
```

### EntitySnapshot

```typescript
{
  entity_id: number;
  entity_type: string;  // "player:{address}" or "npc:{type}"
  position: [number, number, number];
  health?: [number, number, number];  // [current, max, is_dead]
}
```

### SessionInfo

```typescript
{
  session_id: string;
  address: string;
  connected_at: number;  // unix timestamp
  last_activity: number;
  is_active: boolean;
}
```

## Versioning

- Protocol version is embedded in `admin_nodeInfo` response (`name` field)
- Breaking changes require major version bump
- New methods/events are backward-compatible (minor version)
- Client SDK version should match protocol version for full compatibility

## Error Codes

| Code | Meaning |
|------|---------|
| -32700 | Parse error |
| -32600 | Invalid request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |
| -32000 | Server error (custom) |

## Client Connection Flow

```
1. Client → Node:  game_getWorldInfo()          // Check node is alive
2. Client → Node:  game_connectPlayer(addr, sig) // Authenticate
3. Client → Node:  WS connect                    // Open WebSocket
4. Client → Node:  velochain_subscribeGameTicks   // Subscribe events
5. Client → Node:  velochain_subscribePlayerState
6. Client → Node:  velochain_subscribeChatMessages
7. Client → Node:  game_submitAction("move", ...) // Start playing
8. Client → Node:  game_heartbeat(sessionId)      // Every 10s
...
N. Client → Node:  game_disconnectPlayer(sid)     // Disconnect
```
