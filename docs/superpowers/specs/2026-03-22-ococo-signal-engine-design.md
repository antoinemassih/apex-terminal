# OCOCO Signal Engine — Design Spec

**Date:** 2026-03-22
**Scope:** Real-time signal engine backend for the Apex Terminal trading platform

## Goals

- Unified annotation storage for user drawings and programmatic signals (GEX, patterns, S/D zones, auto-trends)
- Sub-millisecond signal delivery via WebSocket + Redis pub/sub
- Hit detection and price alerts with configurable conditions
- Rich metadata: strength, groups, tags, multi-timeframe visibility, TTL
- Extensible for future compute workers (pattern detection, auto-trend, etc.)

## Architecture

```
Compute Workers (GEX, Pattern, S/D, Auto-Trend, Indicators)
    ↓ publish to Redis channels: signals:{symbol}
Redis Pub/Sub (signal bus)
    ↓
OCOCO API (Fastify + TypeScript)
    ├── REST: /api/annotations CRUD (persistent)
    ├── REST: /api/alerts CRUD
    ├── WS: real-time signal push + alert triggers
    ├── Redis cache: hot annotations per symbol
    ├── PostgreSQL: annotations + alert_rules tables
    └── Hit detection: in-process price monitoring
    ↓
Apex Terminal (Tauri app) connects via WS + REST
```

## Database Schema

### annotations table
```sql
CREATE TABLE annotations (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    symbol          VARCHAR(20) NOT NULL,
    source          VARCHAR(30) NOT NULL DEFAULT 'user',
    type            VARCHAR(30) NOT NULL,
    points          JSONB NOT NULL DEFAULT '[]',
    style           JSONB NOT NULL DEFAULT '{}',
    strength        REAL NOT NULL DEFAULT 0.5,
    "group"         VARCHAR(50),
    tags            TEXT[] NOT NULL DEFAULT '{}',
    visibility      TEXT[] NOT NULL DEFAULT '{*}',
    timeframe       VARCHAR(10),
    ttl             TIMESTAMPTZ,
    metadata        JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_annotations_symbol ON annotations (symbol);
CREATE INDEX idx_annotations_symbol_source ON annotations (symbol, source);
CREATE INDEX idx_annotations_group ON annotations ("group");
CREATE INDEX idx_annotations_tags ON annotations USING GIN (tags);
CREATE INDEX idx_annotations_ttl ON annotations (ttl) WHERE ttl IS NOT NULL;
```

### alert_rules table
```sql
CREATE TABLE alert_rules (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    annotation_id   UUID REFERENCES annotations(id) ON DELETE CASCADE,
    symbol          VARCHAR(20) NOT NULL,
    condition       VARCHAR(30) NOT NULL,
    price           REAL,
    active          BOOLEAN NOT NULL DEFAULT true,
    last_triggered  TIMESTAMPTZ,
    cooldown_sec    INTEGER NOT NULL DEFAULT 60,
    notify          JSONB NOT NULL DEFAULT '{"websocket": true}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_alert_rules_symbol ON alert_rules (symbol) WHERE active = true;
CREATE INDEX idx_alert_rules_annotation ON alert_rules (annotation_id);
```

## API Endpoints

### REST

```
GET    /api/annotations?symbol=AAPL&source=user&group=key-levels&tags=support
POST   /api/annotations                    — create annotation
PATCH  /api/annotations/:id                — update (partial)
PATCH  /api/annotations/:id/points         — update points only (high-freq)
PATCH  /api/annotations/:id/style          — update style only
DELETE /api/annotations/:id
DELETE /api/annotations?source=gex&symbol=AAPL  — batch delete by filter

GET    /api/alerts?symbol=AAPL
POST   /api/alerts                         — create alert rule
PATCH  /api/alerts/:id                     — update
DELETE /api/alerts/:id

GET    /health
```

### WebSocket (ws://ococo-dev.xllio.com/ws)

Client → Server:
```json
{"type": "subscribe", "symbols": ["AAPL", "SPY"]}
{"type": "unsubscribe", "symbols": ["SPY"]}
{"type": "price", "symbol": "AAPL", "price": 185.50, "time": 1711108200}
```

Server → Client:
```json
{"type": "snapshot", "symbol": "AAPL", "annotations": [...]}
{"type": "signal", "annotation": {...}}
{"type": "signal_remove", "id": "uuid", "symbol": "AAPL"}
{"type": "alert", "rule_id": "...", "annotation_id": "...", "symbol": "AAPL", "price": 185.50}
```

## Tech Stack

- **Runtime:** Node.js 22 + TypeScript
- **Framework:** Fastify (REST + WebSocket via @fastify/websocket)
- **Database:** PostgreSQL (ococo database, 192.168.1.143:5432)
- **Cache + Pub/Sub:** Redis (192.168.1.89:6379)
- **ORM:** Raw SQL via pg (node-postgres) — no ORM overhead for performance
- **Deployment:** K3s via k3s-app, dev namespace

## Deployment

```yaml
app: ococo
domain: ococo
services:
  api:
    dockerfile: Dockerfile
    port: 3000
    env_secret: ococo-api-env
    health: /api/health
    routes:
      - path: /
infrastructure:
  postgresql:
    type: ExternalName
    host: 192.168.1.143
    port: 5432
  redis:
    type: ExternalName
    host: 192.168.1.89
    port: 6379
```
