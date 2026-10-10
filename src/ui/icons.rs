// SPDX-License-Identifier: AGPL-3.0-only
//! Application-owned icon binding; labels remain readable before pack registration.
use serde::Deserialize;
use std::collections::HashMap;
use teloxide::types::{
    CustomEmojiId, KeyboardButton, RichText, RichTextCustomEmoji, RichTextObject,
};

#[derive(Clone, Debug, Deserialize)]
struct Icon {
    custom_emoji_id: String,
    emoji: String,
}

/// Manifest-derived custom emoji metadata compiled into the scratch binary.
#[derive(Clone, Debug)]
pub struct IconCatalog(HashMap<String, Icon>);

impl IconCatalog {
    /// Loads the generated mapping; registration requires a binary rebuild.
    /// # Errors
    /// Returns an error for corrupt metadata or invalid nonempty Telegram IDs.
    pub fn bundled() -> anyhow::Result<Self> {
        Self::from_json(include_str!("../../assets/icons/generated/mapping.json"))
    }
    fn from_json(json: &str) -> anyhow::Result<Self> {
        let map: HashMap<String, Icon> = serde_json::from_str(json)?;
        for icon in map.values() {
            anyhow::ensure!(
                icon.custom_emoji_id.is_empty()
                    || icon.custom_emoji_id.bytes().all(|c| c.is_ascii_digit()),
                "invalid custom emoji ID"
            );
            anyhow::ensure!(!icon.emoji.is_empty(), "missing custom emoji alternative");
        }
        Ok(Self(map))
    }
    /// Native reply-keyboard button using the same registered semantic artwork.
    #[must_use]
    pub fn keyboard_button(&self, id: &str, text: &str) -> KeyboardButton {
        let mut button = KeyboardButton::new(text);
        button.icon_custom_emoji_id = self
            .0
            .get(id)
            .filter(|icon| !icon.custom_emoji_id.is_empty())
            .map(|icon| CustomEmojiId(icon.custom_emoji_id.clone()));
        button
    }
    /// Adds registered artwork while preserving the complete action text.
    #[must_use]
    pub fn label(&self, id: &str, text: RichText) -> RichText {
        let Some(icon) = self.0.get(id).filter(|i| !i.custom_emoji_id.is_empty()) else {
            return text;
        };
        RichText::List(vec![
            RichText::Object(RichTextObject::CustomEmoji(RichTextCustomEmoji {
                custom_emoji_id: icon.custom_emoji_id.clone(),
                alternative_text: icon.emoji.clone(),
            })),
            RichText::from(" "),
            text,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_binding_is_text_only_registered_binding_preserves_action_label() {
        IconCatalog::bundled().unwrap();
        let catalog =
            IconCatalog::from_json(r#"{"action.accept":{"custom_emoji_id":"","emoji":"✅"}}"#)
                .unwrap();
        assert_eq!(
            catalog.label("action.accept", RichText::from("Принять")),
            RichText::from("Принять")
        );
        let bound =
            IconCatalog::from_json(r#"{"action.accept":{"custom_emoji_id":"123","emoji":"✅"}}"#)
                .unwrap();
        let json =
            serde_json::to_value(bound.label("action.accept", RichText::from("Принять"))).unwrap();
        assert_eq!(json[0]["custom_emoji_id"], "123");
        assert_eq!(json[2], "Принять");
    }
}
