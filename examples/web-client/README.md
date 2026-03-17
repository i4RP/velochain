# VeloChain Sample Web Client

A minimal browser-based game world viewer for VeloChain nodes.

## Features

- **2D Minimap** — Real-time visualization of players and entities in the game world
- **Player List** — Online players with position and health status
- **Chat** — Send and receive chat messages
- **World Info** — Live stats (tick, block, entity count, sessions)
- **Zoom/Pan** — Mouse wheel zoom, keyboard controls

## Usage

1. Start a VeloChain node:
   ```bash
   velochain run --rpc-addr 127.0.0.1:8545 --validator
   ```

2. Open `index.html` in a browser

3. Enter the RPC URL (default: `http://localhost:8545`) and click **Connect**

## Architecture

This is a single-file HTML application with no build step required. It communicates with the VeloChain node via JSON-RPC over HTTP, polling for updates every second.

For production use, consider using the `@velochain/client-sdk` package with WebSocket subscriptions for lower latency.

## Requirements

- Modern browser (Chrome, Firefox, Safari, Edge)
- A running VeloChain node with RPC enabled
- CORS must be enabled on the node (default: allowed)
