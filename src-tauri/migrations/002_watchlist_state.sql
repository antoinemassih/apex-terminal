-- Watchlist + symbol-universe schema — phase (c) of the watchlist refactor.
--
-- Mirrors `001_chart_state.sql`: BEGIN/COMMIT, defensive drops, indexes inline.
-- The renderer continues to read JSON from disk as a fallback; this Postgres
-- layer is the new canonical store. Universes (S&P 500 sectors, Dow 30,
-- QQQ100, etc.) live in their own tables so they can be refreshed
-- independently from user-owned watchlists.
--
-- Apply with:
--   psql "postgresql://postgres:monkeyxx@192.168.1.143:5432/ococo" \
--     -f migrations/002_watchlist_state.sql

BEGIN;

-- ─────────────────────────────────────────────────────────────────────────
-- 1. Defensive drops — re-running this migration is safe.
-- ─────────────────────────────────────────────────────────────────────────
DROP TABLE IF EXISTS symbol_universe_members CASCADE;
DROP TABLE IF EXISTS symbol_universes CASCADE;
DROP TABLE IF EXISTS watchlist_items CASCADE;
DROP TABLE IF EXISTS watchlist_sections CASCADE;
DROP TABLE IF EXISTS watchlists CASCADE;

-- ─────────────────────────────────────────────────────────────────────────
-- 2. Watchlists (top-level, one row per saved watchlist)
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE watchlists (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         BIGINT NOT NULL DEFAULT 0,
    name            TEXT NOT NULL,
    kind            SMALLINT NOT NULL DEFAULT 0,           -- 0=user 1=system_universe
    is_active       BOOLEAN NOT NULL DEFAULT FALSE,        -- which watchlist is currently selected in the tab strip
    position        INT NOT NULL DEFAULT 0,                 -- ordering in the tab strip
    columns_json    JSONB NOT NULL DEFAULT '[]'::jsonb,     -- per-watchlist column override; [] = use global
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX watchlists_user ON watchlists(user_id, position);

-- ─────────────────────────────────────────────────────────────────────────
-- 3. Sections within a watchlist
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE watchlist_sections (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    watchlist_id    UUID NOT NULL REFERENCES watchlists(id) ON DELETE CASCADE,
    title           TEXT NOT NULL DEFAULT '',
    color           TEXT,
    collapsed       BOOLEAN NOT NULL DEFAULT FALSE,
    position        INT NOT NULL DEFAULT 0
);
CREATE INDEX watchlist_sections_wl ON watchlist_sections(watchlist_id, position);

-- ─────────────────────────────────────────────────────────────────────────
-- 4. Items inside a section (stocks or option contracts)
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE watchlist_items (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    section_id      UUID NOT NULL REFERENCES watchlist_sections(id) ON DELETE CASCADE,
    symbol          TEXT NOT NULL,
    asset_class     SMALLINT NOT NULL DEFAULT 0,            -- match charts.asset_class
    pinned          BOOLEAN NOT NULL DEFAULT FALSE,
    note            TEXT,
    position        INT NOT NULL DEFAULT 0,
    -- option contract fields (NULLable, only populated when asset_class=3)
    is_option       BOOLEAN NOT NULL DEFAULT FALSE,
    underlying      TEXT,
    option_type     TEXT,
    strike          REAL,
    expiry          TEXT
);
CREATE INDEX watchlist_items_section ON watchlist_items(section_id, position);

-- ─────────────────────────────────────────────────────────────────────────
-- 5. Symbol universes (S&P 500, Dow 30, QQQ100, sector ETF holdings, etc.)
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE symbol_universes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind            TEXT NOT NULL,         -- 'index_constituents', 'etf_holdings', 'sector', etc.
    name            TEXT NOT NULL,         -- 'sp500', 'dow30', 'qqq100', 'xlk_holdings'
    display_name    TEXT NOT NULL,         -- 'S&P 500', 'Dow 30', 'XLK Technology'
    source          TEXT,                  -- 'static_seed', 'apex_ib', 'manual'
    fetched_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX symbol_universes_name ON symbol_universes(name);

-- ─────────────────────────────────────────────────────────────────────────
-- 6. Members of a universe
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE symbol_universe_members (
    universe_id     UUID NOT NULL REFERENCES symbol_universes(id) ON DELETE CASCADE,
    symbol          TEXT NOT NULL,
    weight          REAL,
    position        INT NOT NULL DEFAULT 0,
    PRIMARY KEY (universe_id, symbol)
);
CREATE INDEX symbol_universe_members_pos ON symbol_universe_members(universe_id, position);

COMMIT;
