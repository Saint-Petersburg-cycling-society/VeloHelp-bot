// SPDX-License-Identifier: AGPL-3.0-only

//! UI/session state. Domain truth lives in storage; this layer holds wizard
//! steps, temporary selections, view identity and capability metadata.

use serde::{Deserialize, Serialize};

use crate::domain::{GeoPoint, HelpKind};

/// Steps of the request wizard (SPEC §6).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RequestStep {
    Description,
    Location,
    Address,
    Preview,
}

/// Mutable draft for a request wizard. Never authoritative.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RequestDraft {
    pub kind: Option<HelpKind>,
    pub tags: Vec<String>,
    pub description: String,
    pub bike_description: Option<String>,
    pub exact_point: Option<GeoPoint>,
    pub exact_address: Option<String>,
    pub area_id: Option<String>,
    pub step: Option<RequestStep>,
    /// Double-click guard: set after the first publish attempt.
    pub publish_attempted: bool,
}

impl RequestDraft {
    /// Advances the wizard, staying on preview.
    pub fn next(&mut self) {
        self.step = Some(match self.step {
            None => RequestStep::Description,
            Some(RequestStep::Description) => RequestStep::Location,
            Some(RequestStep::Location | RequestStep::Address | RequestStep::Preview) => {
                RequestStep::Preview
            }
        });
    }

    /// Moves back, staying on the first step.
    pub fn back(&mut self) {
        self.step = Some(match self.step {
            None | Some(RequestStep::Description | RequestStep::Location) => {
                RequestStep::Description
            }
            Some(RequestStep::Preview) => RequestStep::Location,
            Some(RequestStep::Address) => RequestStep::Preview,
        });
    }

    /// True when the draft has enough data to publish.
    #[must_use]
    pub fn can_publish(&self) -> bool {
        self.kind.is_some()
            && self
                .exact_address
                .as_ref()
                .is_none_or(|text| !text.trim().is_empty() && text.chars().count() <= 200)
            && !self.description.trim().is_empty()
            && self.description.chars().count() <= 4000
            && self
                .bike_description
                .as_ref()
                .is_none_or(|text| text.chars().count() <= 2000)
            && self
                .area_id
                .as_ref()
                .is_some_and(|area| !area.trim().is_empty())
            && self
                .exact_point
                .is_some_and(|p| GeoPoint::new(p.lat, p.lon).is_some())
            && !self.publish_attempted
    }

    /// Marks the first publish attempt (double-click guard).
    pub fn mark_publish_attempted(&mut self) {
        self.publish_attempted = true;
    }
}

/// Mutable draft for helper setup (SPEC §5).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HelperDraft {
    pub areas: Vec<String>,
    pub kinds: Vec<HelpKind>,
    pub enabled: bool,
    #[serde(default)]
    pub problem_tags: Vec<String>,
}

impl HelperDraft {
    /// Toggles one area id.
    pub fn toggle_area(&mut self, area: &str) {
        if let Some(pos) = self.areas.iter().position(|a| a == area) {
            self.areas.remove(pos);
        } else {
            self.areas.push(area.to_owned());
        }
    }

    /// Toggles one help kind.
    pub fn toggle_kind(&mut self, kind: HelpKind) {
        let all_selected = crate::domain::HELP_PROBLEMS
            .iter()
            .filter(|tag| crate::domain::problem_kind(tag) == Some(kind))
            .all(|tag| self.problem_tags.iter().any(|selected| selected == tag));
        self.problem_tags
            .retain(|tag| crate::domain::problem_kind(tag) != Some(kind));
        if !all_selected {
            self.problem_tags.extend(
                crate::domain::HELP_PROBLEMS
                    .iter()
                    .filter(|tag| crate::domain::problem_kind(tag) == Some(kind))
                    .map(|tag| (*tag).to_owned()),
            );
        }
        self.sync_kinds();
    }
    /// Toggles a problem and derives kind eligibility from the selected problems.
    pub fn toggle_problem(&mut self, tag: &str) {
        if let Some(index) = self.problem_tags.iter().position(|t| t == tag) {
            self.problem_tags.remove(index);
        } else {
            self.problem_tags.push(tag.to_owned());
        }
        self.sync_kinds();
    }
    fn sync_kinds(&mut self) {
        self.kinds = [HelpKind::Technical, HelpKind::Accident]
            .into_iter()
            .filter(|kind| {
                self.problem_tags
                    .iter()
                    .any(|tag| crate::domain::problem_kind(tag) == Some(*kind))
            })
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wizard_preserves_draft_across_navigation() {
        let mut draft = RequestDraft {
            description: "test".to_owned(),
            ..Default::default()
        };
        draft.next();
        draft.next();
        draft.back();
        assert_eq!(draft.step, Some(RequestStep::Description));
        assert_eq!(draft.description, "test");
    }

    #[test]
    fn publish_double_click_is_blocked() {
        let mut draft = RequestDraft {
            kind: Some(HelpKind::Technical),
            description: "x".to_owned(),
            exact_point: GeoPoint::new(59.9, 30.3),
            area_id: Some("spb-center".to_owned()),
            ..Default::default()
        };
        assert!(draft.can_publish());
        draft.mark_publish_attempted();
        assert!(!draft.can_publish());
    }
}
