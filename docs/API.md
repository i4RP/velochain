# VeloChain JSON-RPC API Reference

VeloChain exposes a JSON-RPC 2.0 API over HTTP and WebSocket on the same port (default: `127.0.0.1:8545`).

## Ethereum-Compatible Methods (`eth_` namespace)

### eth_chainId

Returns the chain ID.

```json
{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}
```

Response: `"0x6a2d"` (27181 in decimal)

---

### eth_blockNumber

Returns the latest block number.

```json
{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}
```

Response: `"0x1a"` (hex-encoded block number)

---

### eth_getBalance

Returns the balance of an account.

```json
{"jsonrpc":"2.0","method":"eth_getBalance","params":["0x742d35Cc6634C0532925a3b844Bc9e7595f2bD28","latest"],"id":1}
```

---

### eth_getTransactionCount

Returns the nonce (transaction count) of an account.

```json
{"jsonrpc":"2.0","method":"eth_getTransactionCount","params":["0x742d35Cc6634C0532925a3b844Bc9e7595f2bD28","latest"],"id":1}
```

---

### eth_sendRawTransaction

Submits a signed transaction to the transaction pool.

```json
{"jsonrpc":"2.0","method":"eth_sendRawTransaction","params":["0x<hex-encoded-signed-tx>"],"id":1}
```

Response: Transaction hash.

---

### eth_gasPrice

Returns the current gas price (default: 1 gwei).

```json
{"jsonrpc":"2.0","method":"eth_gasPrice","params":[],"id":1}
```

Response: `"0x3b9aca00"`

---

### eth_getBlockByNumber

Returns block information by number. Use `"latest"` for the most recent block.

```json
{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["latest",false],"id":1}
```

Response includes: `number`, `hash`, `parentHash`, `timestamp`, `gasLimit`, `gasUsed`, `beneficiary`, `stateRoot`, `transactionsRoot`, `receiptsRoot`, `gameTick`, `gameStateRoot`, `difficulty`, `transactions`.

---

### eth_getBlockByHash

Returns block information by hash.

```json
{"jsonrpc":"2.0","method":"eth_getBlockByHash","params":["0x<block-hash>",false],"id":1}
```

---

### eth_getTransactionReceipt

Returns a transaction receipt by hash.

```json
{"jsonrpc":"2.0","method":"eth_getTransactionReceipt","params":["0x<tx-hash>"],"id":1}
```

Response includes: `transactionHash`, `blockNumber`, `blockHash`, `transactionIndex`, `success`, `gasUsed`, `cumulativeGasUsed`, `contractAddress`, `logs`.

---

### eth_call

Executes a read-only call without creating a transaction.

```json
{"jsonrpc":"2.0","method":"eth_call","params":[{"from":"0x...","to":"0x...","data":"0x..."},"latest"],"id":1}
```

---

### eth_estimateGas

Estimates gas for a transaction.

```json
{"jsonrpc":"2.0","method":"eth_estimateGas","params":[{"from":"0x...","to":"0x...","value":"0x0"},"latest"],"id":1}
```

---

### eth_getCode

Returns the code at a given address.

```json
{"jsonrpc":"2.0","method":"eth_getCode","params":["0x<address>","latest"],"id":1}
```

---

### eth_getLogs

Returns logs matching a filter.

```json
{"jsonrpc":"2.0","method":"eth_getLogs","params":[{"fromBlock":"0x0","toBlock":"latest","address":"0x..."}],"id":1}
```

---

### net_version

Returns the network version (chain ID as decimal string).

```json
{"jsonrpc":"2.0","method":"net_version","params":[],"id":1}
```

---

## Game-Specific Methods (`game_` namespace)

### game_getWorldInfo

Returns current world status.

```json
{"jsonrpc":"2.0","method":"game_getWorldInfo","params":[],"id":1}
```

Response:
```json
{
  "current_tick": 1234,
  "entity_count": 50,
  "player_count": 10,
  "seed": 42
}
```

---

### game_getPlayer

Returns player information by address.

```json
{"jsonrpc":"2.0","method":"game_getPlayer","params":["0x742d35Cc..."],"id":1}
```

---

### game_getCurrentTick

Returns the current game tick number.

```json
{"jsonrpc":"2.0","method":"game_getCurrentTick","params":[],"id":1}
```

---

### game_getEntityCount

Returns the total entity count in the game world.

```json
{"jsonrpc":"2.0","method":"game_getEntityCount","params":[],"id":1}
```

---

### game_submitAction

Submits a game action (move, attack, chat, respawn).

Move:
```json
{"jsonrpc":"2.0","method":"game_submitAction","params":["move",{"x":10,"y":0,"z":5}],"id":1}
```

Attack:
```json
{"jsonrpc":"2.0","method":"game_submitAction","params":["attack",{"target_entity_id":42}],"id":1}
```

Chat:
```json
{"jsonrpc":"2.0","method":"game_submitAction","params":["chat",{"message":"Hello world!"}],"id":1}
```

Respawn:
```json
{"jsonrpc":"2.0","method":"game_submitAction","params":["respawn",{}],"id":1}
```

---

### game_getAllPlayers

Returns a list of all players in the world.

```json
{"jsonrpc":"2.0","method":"game_getAllPlayers","params":[],"id":1}
```

---

### game_getEntitiesInArea

Returns entities within a bounding box (min_x, min_y, max_x, max_y).

```json
{"jsonrpc":"2.0","method":"game_getEntitiesInArea","params":[0.0, 0.0, 100.0, 100.0],"id":1}
```

---

## Admin Methods (`admin_` namespace)

> Admin endpoints must be explicitly enabled via `--enable-admin` flag or configuration.

### admin_nodeInfo

Returns detailed node information.

```json
{"jsonrpc":"2.0","method":"admin_nodeInfo","params":[],"id":1}
```

Response:
```json
{
  "name": "VeloChain/v0.1.0",
  "chainId": 27181,
  "blockNumber": 100,
  "gameTick": 500,
  "entityCount": 50,
  "playerCount": 10,
  "pendingTxCount": 3,
  "uptimeSecs": 3600
}
```

---

### admin_health

Returns health check status.

```json
{"jsonrpc":"2.0","method":"admin_health","params":[],"id":1}
```

Response:
```json
{
  "healthy": true,
  "blockNumber": 100,
  "gameTick": 500,
  "pendingTxCount": 3,
  "dbOk": true
}
```

---

### admin_peerCount

Returns the number of connected peers.

```json
{"jsonrpc":"2.0","method":"admin_peerCount","params":[],"id":1}
```

---

## WebSocket Subscriptions

Connect via WebSocket to receive real-time events.

### VeloChain-specific subscriptions (`velochain_` namespace)

Subscribe to new blocks:
```json
{"jsonrpc":"2.0","method":"velochain_subscribeNewBlocks","params":[],"id":1}
```

Subscribe to game ticks:
```json
{"jsonrpc":"2.0","method":"velochain_subscribeGameTicks","params":[],"id":1}
```

Subscribe to pending transactions:
```json
{"jsonrpc":"2.0","method":"velochain_subscribePendingTxs","params":[],"id":1}
```

### Ethereum-standard subscriptions (`eth_` namespace)

New block heads:
```json
{"jsonrpc":"2.0","method":"eth_subscribe","params":["newHeads"],"id":1}
```

New pending transactions:
```json
{"jsonrpc":"2.0","method":"eth_subscribe","params":["newPendingTransactions"],"id":1}
```

### Event Payloads

New Block:
```json
{"type":"newBlock","number":100,"hash":"0x...","txCount":5,"timestamp":1700000000}
```

Game Tick:
```json
{"type":"gameTick","tick":500,"entityCount":50,"playerCount":10,"stateRoot":"0x..."}
```

Pending Transaction:
```json
{"type":"pendingTransaction","hash":"0x..."}
```

---

## Configuration

Default RPC endpoint: `http://127.0.0.1:8545`

Configure via TOML (`velochain.toml`):
```toml
[rpc]
addr = "0.0.0.0:8545"
max_ws_connections = 100
enable_admin = true
```

Or via environment variables:
```bash
export VELOCHAIN_RPC_ADDR="0.0.0.0:8545"
export VELOCHAIN_RPC_ENABLE_ADMIN=true
```

---

## Error Codes

| Code | Description |
|------|-------------|
| -32600 | Invalid request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32000 | Internal server error |
