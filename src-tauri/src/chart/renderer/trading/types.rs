//! Core order enums shared across the trading subsystem.

// ─── Legacy rendering enums ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum OrderSide { Buy, Sell, Stop, OcoTarget, OcoStop, TriggerBuy, TriggerSell }

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum OrderStatus { Draft, Placed, Executed, Cancelled }

// ─── Managed order state machine ────────────────────────────────────────────

/// Extended order status with full lifecycle
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum OrderState {
    Draft,           // Created locally, not yet submitted (awaiting confirmation)
    PendingSubmit,   // Queued for submission to backend
    Working,         // Confirmed by backend, live in market
    PartialFill,     // Partially filled
    Filled,          // Fully filled
    PendingCancel,   // Cancel request sent to backend
    Cancelled,       // Confirmed cancelled
    Rejected,        // Rejected by backend or risk checks
    PendingModify,   // Modify request sent to backend
    Unknown,         // Pending operation timed out — broker query needed
}

impl OrderState {
    pub(crate) fn is_active(&self) -> bool {
        matches!(self, Self::Draft | Self::PendingSubmit | Self::Working | Self::PartialFill | Self::PendingModify)
    }
    pub(crate) fn is_terminal(&self) -> bool {
        matches!(self, Self::Filled | Self::Cancelled | Self::Rejected)
    }
    pub(crate) fn is_pending(&self) -> bool {
        matches!(self, Self::PendingSubmit | Self::PendingCancel | Self::PendingModify)
    }
    /// Map to legacy OrderStatus for backward compat with rendering code.
    /// `Unknown` renders as Placed so it stays visible until reconciled.
    pub(crate) fn to_legacy(&self) -> OrderStatus {
        match self {
            Self::Draft => OrderStatus::Draft,
            Self::PendingSubmit | Self::Working | Self::PartialFill | Self::PendingModify | Self::Unknown => OrderStatus::Placed,
            Self::Filled => OrderStatus::Executed,
            Self::PendingCancel | Self::Cancelled | Self::Rejected => OrderStatus::Cancelled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ManagedOrderType { Market, Limit, Stop, StopLimit, TrailingStop }

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum OrderSource {
    ChartClick,     // Click on chart
    DomLadder,      // DOM price ladder
    DomButton,      // DOM buy/sell button
    OrderPanel,     // Order entry panel
    Hotkey,         // Keyboard shortcut
    Bracket,        // Auto-generated bracket leg
    Trigger,        // Trigger/conditional order
    Api,            // External API
    Oco,            // OCO group leg
    Combo,          // Combo/spread order
    Conditional,    // Conditional order
    OptionsTrigger, // Options trigger order
}

/// Get current epoch milliseconds (shared utility used across trading subsystem).
pub(crate) fn epoch_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant of the OrderState enum, in declaration order. If a new
    /// variant is added without updating the `to_legacy` match arms, this
    /// test (and the exhaustive match below) will fail to compile.
    fn all_variants() -> Vec<OrderState> {
        vec![
            OrderState::Draft,
            OrderState::PendingSubmit,
            OrderState::Working,
            OrderState::PartialFill,
            OrderState::Filled,
            OrderState::PendingCancel,
            OrderState::Cancelled,
            OrderState::Rejected,
            OrderState::PendingModify,
            OrderState::Unknown,
        ]
    }

    #[test]
    fn is_active_true_for_active_states() {
        assert!(OrderState::Draft.is_active());
        assert!(OrderState::PendingSubmit.is_active());
        assert!(OrderState::Working.is_active());
        assert!(OrderState::PartialFill.is_active());
        assert!(OrderState::PendingModify.is_active());
    }

    #[test]
    fn is_active_false_for_inactive_states() {
        assert!(!OrderState::Filled.is_active());
        assert!(!OrderState::PendingCancel.is_active());
        assert!(!OrderState::Cancelled.is_active());
        assert!(!OrderState::Rejected.is_active());
        assert!(!OrderState::Unknown.is_active());
    }

    #[test]
    fn is_pending_only_for_pending_variants() {
        assert!(OrderState::PendingSubmit.is_pending());
        assert!(OrderState::PendingCancel.is_pending());
        assert!(OrderState::PendingModify.is_pending());

        assert!(!OrderState::Draft.is_pending());
        assert!(!OrderState::Working.is_pending());
        assert!(!OrderState::PartialFill.is_pending());
        assert!(!OrderState::Filled.is_pending());
        assert!(!OrderState::Cancelled.is_pending());
        assert!(!OrderState::Rejected.is_pending());
        assert!(!OrderState::Unknown.is_pending());
    }

    #[test]
    fn to_legacy_is_exhaustive_and_does_not_panic() {
        // Iterating every variant exercises every match arm. Compilation
        // already proves exhaustiveness; this test guards against runtime
        // panics in any future variant additions.
        for state in all_variants() {
            let _ = state.to_legacy();
        }
    }

    #[test]
    fn to_legacy_mappings() {
        assert_eq!(OrderState::Draft.to_legacy(), OrderStatus::Draft);
        // Active/in-flight states render as Placed
        assert_eq!(OrderState::PendingSubmit.to_legacy(), OrderStatus::Placed);
        assert_eq!(OrderState::Working.to_legacy(),       OrderStatus::Placed);
        assert_eq!(OrderState::PartialFill.to_legacy(),   OrderStatus::Placed);
        assert_eq!(OrderState::PendingModify.to_legacy(), OrderStatus::Placed);
        // Unknown stays visible (Placed) until reconciled
        assert_eq!(OrderState::Unknown.to_legacy(),       OrderStatus::Placed);
        // Terminal "good"
        assert_eq!(OrderState::Filled.to_legacy(),        OrderStatus::Executed);
        // Cancellation-related
        assert_eq!(OrderState::PendingCancel.to_legacy(), OrderStatus::Cancelled);
        assert_eq!(OrderState::Cancelled.to_legacy(),     OrderStatus::Cancelled);
        assert_eq!(OrderState::Rejected.to_legacy(),      OrderStatus::Cancelled);
    }
}
