//! Codec layer — converts canonical `ChartState` to/from each persistence
//! format. See `docs/CHART_STORAGE_ARCHITECTURE.md` §7.
//!
//! Codec lineup:
//!   - `db`     : Postgres normalized rows ↔ canonical (Phase 3)
//!   - `native` : `.apxchart` binary ↔ canonical (Phase 2)
//!   - `xol`    : zip + JSON ↔ canonical (Phase 4)
//!
//! No legacy adapter — we don't preserve old `drawings`/`drawing_groups`
//! data. The new schema is the truth.

#![allow(dead_code)]

pub mod db;

use std::io::{Read, Write};

use super::ChartState;

/// All format codecs implement this. Phase 2/3/4 modules will provide concrete
/// types; the trait just establishes a uniform shape.
pub trait ChartCodec {
    type Error: std::error::Error;

    fn read(&self, src: &mut dyn Read) -> Result<ChartState, Self::Error>;
    fn write(&self, state: &ChartState, dst: &mut dyn Write) -> Result<(), Self::Error>;
}
