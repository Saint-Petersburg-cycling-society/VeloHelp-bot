// SPDX-License-Identifier: AGPL-3.0-only

mod client;

pub use client::telegram_http_client;

// Telegram transport boundary.
//
// Responsibilities:
// - convert Telegram Update into typed app/UI event;
// - acquire Telegram identity and explicit external inputs;
// - invoke application commands;
// - execute projection effects after commit.
//
// It must not implement request matching or winner selection.
