// SPDX-License-Identifier: AGPL-3.0-only

//! Telegram transport boundary.
//!
//! Responsibilities:
//! - convert Telegram Update into typed app/UI events;
//! - acquire Telegram identity and explicit external inputs;
//! - invoke application commands;
//! - execute projection effects after commit.
//!
//! It must not implement request matching or winner selection.

mod client;
mod handler;

pub use client::telegram_http_client;
pub use handler::{describe_event, ExternalInput, TelegramEvent};
