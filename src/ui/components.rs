// SPDX-License-Identifier: AGPL-3.0-only

//! Generic reusable interaction primitives.
//!
//! No bicycle domain concepts here: no `Bike*` types, no bicycle emoji.
//! Domain composition lives in `views.rs`; mechanics live here.

use serde::{Deserialize, Serialize};

/// Generic multi-select over cloneable options.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiSelect<T>
where
    T: Clone + Eq,
{
    options: Vec<T>,
    selected: Vec<T>,
}

impl<T> MultiSelect<T>
where
    T: Clone + Eq,
{
    /// Creates a selector with the full option list and no selection.
    #[must_use]
    pub fn new(options: Vec<T>) -> Self {
        Self {
            options,
            selected: Vec::new(),
        }
    }

    /// All available options.
    #[must_use]
    pub fn options(&self) -> &[T] {
        &self.options
    }

    /// Currently selected options in selection order.
    #[must_use]
    pub fn selected(&self) -> &[T] {
        &self.selected
    }

    /// Toggles one option. Unknown options are ignored.
    pub fn toggle(&mut self, option: &T) {
        if !self.options.contains(option) {
            return;
        }
        if let Some(pos) = self.selected.iter().position(|s| s == option) {
            self.selected.remove(pos);
        } else {
            self.selected.push(option.clone());
        }
    }

    /// Number of selected options.
    #[must_use]
    pub fn count(&self) -> usize {
        self.selected.len()
    }

    /// True when nothing is selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }
}

/// Generic linear wizard over an ordered step list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wizard<S>
where
    S: Clone + Eq,
{
    steps: Vec<S>,
    current: usize,
}

impl<S> Wizard<S>
where
    S: Clone + Eq,
{
    /// Creates a wizard. Panics on empty steps in debug; falls back to a
    /// single-step guard in release. Prefer non-empty steps.
    #[must_use]
    pub fn new(steps: Vec<S>) -> Self {
        debug_assert!(!steps.is_empty(), "wizard requires steps");
        Self { steps, current: 0 }
    }

    /// Current step, if any.
    #[must_use]
    pub fn current(&self) -> Option<&S> {
        self.steps.get(self.current)
    }

    /// Zero-based step index.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.current
    }

    /// Total step count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// True when no steps exist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// True on the first step.
    #[must_use]
    pub fn is_first(&self) -> bool {
        self.current == 0
    }

    /// True on the last step.
    #[must_use]
    pub fn is_last(&self) -> bool {
        self.steps.is_empty() || self.current + 1 >= self.steps.len()
    }

    /// Moves forward, staying on the last step.
    pub fn next(&mut self) {
        if !self.steps.is_empty() && self.current + 1 < self.steps.len() {
            self.current += 1;
        }
    }

    /// Moves back, staying on the first step.
    pub fn back(&mut self) {
        self.current = self.current.saturating_sub(1);
    }
}

/// Generic two-step confirmation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Confirm {
    Pending,
    Confirmed,
    Cancelled,
}

impl Confirm {
    /// Confirms a pending dialog; terminal states are sticky.
    #[must_use]
    pub const fn confirm(self) -> Self {
        match self {
            Self::Pending => Self::Confirmed,
            other => other,
        }
    }

    /// Cancels a pending dialog; terminal states are sticky.
    #[must_use]
    pub const fn cancel(self) -> Self {
        match self {
            Self::Pending => Self::Cancelled,
            other => other,
        }
    }
}

/// Generic status badge (text + icon id resolved by the app layer).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StatusBadge {
    Open,
    Matched,
    Resolved,
    Cancelled,
    Expired,
}

impl StatusBadge {
    /// Stable icon id from `assets/icons/manifest.toml`.
    #[must_use]
    pub const fn icon_id(self) -> &'static str {
        match self {
            Self::Open => "status.open",
            Self::Matched => "status.matched",
            Self::Resolved => "status.resolved",
            // No dedicated cancelled icon in DESIGN_SYSTEM; reuse expired.
            Self::Cancelled | Self::Expired => "status.expired",
        }
    }

    /// Accessible text label (icon is never the sole carrier of meaning).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Matched => "Matched",
            Self::Resolved => "Resolved",
            Self::Cancelled => "Cancelled",
            Self::Expired => "Expired",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiselect_toggles_rapidly() {
        let mut select = MultiSelect::new(vec!["a".to_owned(), "b".to_owned()]);
        for _ in 0..10 {
            select.toggle(&"a".to_owned());
        }
        assert!(select.is_empty(), "even toggles cancel out");
        select.toggle(&"unknown".to_owned());
        assert!(select.is_empty(), "unknown options are ignored");
    }

    #[test]
    fn wizard_back_forward_preserves_position() {
        let mut wizard = Wizard::new(vec!["kind", "tags", "preview"]);
        assert!(wizard.is_first());
        wizard.back();
        assert!(wizard.is_first());
        wizard.next();
        wizard.next();
        assert!(wizard.is_last());
        wizard.next();
        assert!(wizard.is_last());
        wizard.back();
        assert_eq!(wizard.current(), Some(&"tags"));
    }

    #[test]
    fn confirm_is_sticky() {
        assert_eq!(Confirm::Pending.confirm(), Confirm::Confirmed);
        assert_eq!(Confirm::Confirmed.cancel(), Confirm::Confirmed);
    }
}
