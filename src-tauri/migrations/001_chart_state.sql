-- Chart state schema — see docs/CHART_STORAGE_ARCHITECTURE.md §4
--
-- This is the new canonical storage. The old `drawings` and `drawing_groups`
-- tables are dropped at the bottom — we don't migrate old data.
--
-- Apply with:
--   psql "postgresql://postgres:monkeyxx@192.168.1.143:5432/ococo" \
--     -f migrations/001_chart_state.sql

BEGIN;

-- ─────────────────────────────────────────────────────────────────────────
-- 1. Drop legacy
-- ─────────────────────────────────────────────────────────────────────────
DROP TABLE IF EXISTS drawings CASCADE;
DROP TABLE IF EXISTS drawing_groups CASCADE;
-- Defensive drops in case a prior partial run of this migration left tables behind.
-- Safe because none of these are populated by anything other than this migration.
DROP TABLE IF EXISTS chart_annotations CASCADE;
DROP TABLE IF EXISTS chart_styles CASCADE;
DROP TABLE IF EXISTS chart_revisions CASCADE;
DROP TABLE IF EXISTS import_warnings CASCADE;
DROP TABLE IF EXISTS indicator_refs CASCADE;
DROP TABLE IF EXISTS pane_templates CASCADE;
DROP TABLE IF EXISTS charts CASCADE;

-- ─────────────────────────────────────────────────────────────────────────
-- 2. Charts (the top-level entity)
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE charts (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id           BIGINT NOT NULL DEFAULT 0,           -- single-tenant for now
    title             TEXT,
    symbol_canonical  TEXT NOT NULL,
    asset_class       SMALLINT NOT NULL,                   -- 0=equity 1=etf 2=index 3=option 4=future 5=crypto 6=fx
    timeframe         SMALLINT NOT NULL,                   -- enum discriminant
    theme             SMALLINT NOT NULL DEFAULT 0,
    viewport          BYTEA NOT NULL,                      -- packed: 2×i64 + 2×f32 + 1 byte flags
    schema_version    INT NOT NULL DEFAULT 1,
    template_id       UUID,                                -- FK added below after pane_templates exists
    head_revision_id  UUID,                                -- FK added below after chart_revisions exists
    description       TEXT,
    extras            JSONB NOT NULL DEFAULT '{}'::jsonb,  -- ExtensionBag
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX charts_user_symbol ON charts(user_id, symbol_canonical);

-- ─────────────────────────────────────────────────────────────────────────
-- 3. Per-chart interned style table
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE chart_styles (
    chart_id     UUID NOT NULL REFERENCES charts(id) ON DELETE CASCADE,
    style_id     INT  NOT NULL,            -- index into the chart's style array
    stroke       INT  NOT NULL,            -- 0xRRGGBBAA packed
    width_x100   SMALLINT NOT NULL,        -- 1.5 stored as 150
    dash         SMALLINT NOT NULL,        -- 0=solid 1=dashed 2=dotted
    fill         INT NOT NULL DEFAULT 0,
    PRIMARY KEY (chart_id, style_id)
);

-- ─────────────────────────────────────────────────────────────────────────
-- 4. Drawings (one row per primitive)
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE drawings (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chart_id     UUID NOT NULL REFERENCES charts(id) ON DELETE CASCADE,
    kind         SMALLINT NOT NULL,        -- DrawingKind discriminant
    z            SMALLINT NOT NULL DEFAULT 0,
    flags        SMALLINT NOT NULL,        -- bitflags
    style_id     INT NOT NULL,             -- references chart_styles.style_id (same chart)
    points       BYTEA NOT NULL,           -- packed; see §4.1
    extras       JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX drawings_chart ON drawings(chart_id);

-- ─────────────────────────────────────────────────────────────────────────
-- 5. Annotations
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE chart_annotations (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chart_id     UUID NOT NULL REFERENCES charts(id) ON DELETE CASCADE,
    anchor_ts_ns BIGINT NOT NULL,
    anchor_price REAL   NOT NULL,
    title        TEXT NOT NULL,
    body_md      TEXT NOT NULL,
    color        INT  NOT NULL,
    asset_refs   TEXT[] NOT NULL DEFAULT '{}',
    extras       JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX chart_annotations_chart ON chart_annotations(chart_id);

-- ─────────────────────────────────────────────────────────────────────────
-- 6. Indicator references on a chart
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE indicator_refs (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chart_id              UUID NOT NULL REFERENCES charts(id) ON DELETE CASCADE,
    ref_id                TEXT NOT NULL,
    ref_version           INT  NOT NULL,
    param_schema_version  INT  NOT NULL,
    pane                  TEXT NOT NULL,
    params                JSONB NOT NULL,
    style                 JSONB,
    installed_locally     BOOLEAN NOT NULL DEFAULT TRUE,
    extras                JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX indicator_refs_ref   ON indicator_refs(ref_id);
CREATE INDEX indicator_refs_chart ON indicator_refs(chart_id);

-- ─────────────────────────────────────────────────────────────────────────
-- 7. Pane templates ("Day Trader", etc.)
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE pane_templates (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             BIGINT NOT NULL DEFAULT 0,
    name                TEXT NOT NULL,
    version             INT NOT NULL DEFAULT 1,
    panes               JSONB NOT NULL,
    default_indicators  JSONB NOT NULL,
    extras              JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, name)
);

-- ─────────────────────────────────────────────────────────────────────────
-- 8. Revision history (binary diffs vs parent + periodic snapshots)
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE chart_revisions (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chart_id             UUID NOT NULL REFERENCES charts(id) ON DELETE CASCADE,
    parent_id            UUID REFERENCES chart_revisions(id),
    author_id            BIGINT NOT NULL DEFAULT 0,
    diff                 BYTEA,
    full_snapshot        BYTEA,                 -- non-null on every Nth revision
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX chart_revisions_chart ON chart_revisions(chart_id);

-- ─────────────────────────────────────────────────────────────────────────
-- 9. Drawing groups (UX feature — colored organizational labels)
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE chart_drawing_groups (
    id          TEXT PRIMARY KEY,         -- caller-supplied id; "default" reserved
    user_id     BIGINT NOT NULL DEFAULT 0,
    name        TEXT NOT NULL,
    color       TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ─────────────────────────────────────────────────────────────────────────
-- 10. Import warnings (non-blocking issues from XOL imports)
-- ─────────────────────────────────────────────────────────────────────────
CREATE TABLE import_warnings (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chart_id    UUID NOT NULL REFERENCES charts(id) ON DELETE CASCADE,
    kind        SMALLINT NOT NULL,           -- 1=missing_indicator 2=newer_indicator 3=missing_in_template ...
    ref_id      TEXT,
    detail      JSONB,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX import_warnings_chart ON import_warnings(chart_id);

-- ─────────────────────────────────────────────────────────────────────────
-- 10. Cross-table FKs that needed both sides to exist first
-- ─────────────────────────────────────────────────────────────────────────
ALTER TABLE charts
    ADD CONSTRAINT charts_template_fk
    FOREIGN KEY (template_id) REFERENCES pane_templates(id) ON DELETE SET NULL;

ALTER TABLE charts
    ADD CONSTRAINT charts_head_revision_fk
    FOREIGN KEY (head_revision_id) REFERENCES chart_revisions(id) ON DELETE SET NULL;

COMMIT;
