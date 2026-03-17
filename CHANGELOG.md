# Changelog

All notable changes to VeloChain will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-17

### Added

#### Core Infrastructure (Phases 1-4)
- Workspace scaffolding with 11 crates: primitives, storage, state, consensus, network, evm, game-engine, txpool, rpc, node, node-cli
- RocksDB-backed persistent storage with column families (headers, bodies, receipts, game state, meta)
- World state management with account caching, dirty tracking, and state root computation
- Proof-of-Authority (Clique-style) consensus with round-robin validator selection
- EVM execution via revm with transaction processing, gas accounting, and state flush
- Game engine with ECS architecture, deterministic ticking, and player/NPC management
- Transaction pool with nonce validation, gas-price ordering, and pending/queued separation
- JSON-RPC server (eth_*, game_*, admin_*) with WebSocket subscriptions
- Block production pipeline with gas limit enforcement and transactions_root computation
- Transaction receipts with storage and retrieval
- Nonce management and gas cost deduction for both EVM and game action transactions

#### Networking & Operations (Phases 5-7)
- P2P networking via libp2p (TCP/Noise/Yamux/GossipSub/Kademlia/mDNS)
- Chain sync protocol (GetHeaders, GetBodies, Status messages)
- Validator management with dynamic validator set updates
- Prometheus metrics (blocks, transactions, peers, game ticks, latencies)
- Graceful shutdown with SIGINT/SIGTERM handling and game world persistence
- State snapshot export/import in portable binary format
- Admin RPC endpoints (admin_nodeInfo, admin_health, admin_peerCount)
- TOML configuration file with VELOCHAIN_* environment variable overrides
- Chain reorganization detection and execution
- Structured logging with per-module log levels

#### Testing & Performance (Phase 8)
- Comprehensive unit tests: consensus (16), txpool (15), evm (9), storage (14), genesis (12), cache (8)
- LRU cache layer (ChainCache) for blocks, headers, and receipts
- Atomic batch write operations for storage
- API documentation (docs/API.md)
- Docker support with multi-stage Dockerfile and docker-compose.yml

#### Client SDK & Protocol (Phase 9)
- TypeScript/JavaScript client SDK (@velochain/client-sdk)
- WebSocket real-time subscriptions (PlayerState, ChatMessage, EntityUpdate)
- Session management with SessionManager and SessionApi RPC
- Client protocol specification (docs/CLIENT_PROTOCOL.md)
- Sample 2D web client with minimap, chat, and player list

#### Game Content (Phase 10)
- Procedural world generation with 12 tile types, 8 biomes, 16x16 chunk system
- Item and inventory system with 32 items, 5 rarities, 6 categories
- NPC AI with 6 archetypes, 6 behavior patterns, and SpawnManager
- Combat system with damage calculation, criticals, range, cooldowns, and XP scaling
- Game events: day/night cycle, weather changes, enemy waves, ground item drops

#### 3D Web Client (Phase 11)
- Bevy 0.15 engine with WASM target support
- Tile-based terrain rendering with dynamic chunk loading
- Entity rendering for players, NPCs, and items
- UI/HUD: health bar, inventory panel, chat log, debug overlay
- Network/SDK integration with ClientAction system

#### Game Systems (Phase 12)
- Crafting system with recipe definitions and material consumption
- Player-to-player trading with on-chain safe exchange protocol
- Quest system with quest definitions, progress tracking, and rewards
- Skill system with skill trees and level-up skill point allocation
- NPC shops with merchant NPCs and dynamic pricing

#### Multi-Node & Network (Phase 13)
- Validator manager with add/remove validator operations
- Storage pruning for old blocks and receipts
- Peer manager with connection limits and peer scoring
- Chain synchronization protocol for new node bootstrap
- Multi-node integration tests

#### Client Enhancement (Phase 14)
- Enhanced UI panels (crafting, trading, quest log, skill tree, shop)
- Camera effects (screen shake, zoom)
- Particle system for visual feedback
- Sound system stubs for BGM and sound effects
- Touch/mobile input support with virtual joystick

#### Integration & Release (Phase 15)
- 33 cross-crate integration tests covering full chain pipeline
- End-to-end node startup, genesis init, and 10-block production test
- Configuration validation and defaults tests
- GitHub Actions CI pipeline (check, clippy, test, fmt, release build)
- Release build profile optimization (LTO, single codegen unit, symbol stripping)

### Architecture

- **Chain ID**: 27181 (devnet), 27182 (testnet)
- **Consensus**: Proof-of-Authority with round-robin validator selection
- **Block time**: 1 second (configurable)
- **Game tick**: 1 tick per block (200ms tick interval for 5 ticks/second)
- **Block gas limit**: 30,000,000 (configurable)
- **Game action gas**: 21,000 fixed per action
- **Storage**: RocksDB with column families
- **Networking**: libp2p (TCP + Noise + Yamux)
- **EVM**: revm with full Ethereum compatibility
- **Game engine**: Custom ECS with deterministic ticking

[0.1.0]: https://github.com/i4RP/velochain/releases/tag/v0.1.0
