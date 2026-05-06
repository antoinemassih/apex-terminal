//! Per-chart interned style table. Most drawings reuse 2–5 styles, so storing
//! them once and referring to them by id collapses both memory and DB row size.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StyleId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashKind {
    Solid,
    Dashed,
    Dotted,
}

impl Default for DashKind {
    fn default() -> Self {
        DashKind::Solid
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Style {
    /// 0xRRGGBBAA packed.
    pub stroke: u32,
    /// Width × 100 (e.g., 1.5 stored as 150) — keeps it integer for hashing.
    pub width_x100: u16,
    pub dash: DashKind,
    /// Optional fill color (0xRRGGBBAA), 0 = no fill.
    pub fill: u32,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct StyleTable {
    styles: Vec<Style>,
}

impl StyleTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a style; returns the existing id if a matching entry is present.
    pub fn intern(&mut self, style: Style) -> StyleId {
        if let Some(idx) = self.styles.iter().position(|s| *s == style) {
            return StyleId(idx as u32);
        }
        let id = StyleId(self.styles.len() as u32);
        self.styles.push(style);
        id
    }

    pub fn get(&self, id: StyleId) -> Option<&Style> {
        self.styles.get(id.0 as usize)
    }

    pub fn len(&self) -> usize {
        self.styles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_dedups() {
        let mut t = StyleTable::new();
        let s = Style { stroke: 0xFFB800FF, width_x100: 150, dash: DashKind::Solid, fill: 0 };
        let a = t.intern(s);
        let b = t.intern(s);
        assert_eq!(a, b);
        assert_eq!(t.len(), 1);
    }
}
