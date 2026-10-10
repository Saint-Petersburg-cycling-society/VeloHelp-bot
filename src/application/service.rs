// SPDX-License-Identifier: AGPL-3.0-only

use async_trait::async_trait;
use thiserror::Error;

use crate::domain::{AcceptOutcome, HelpRequest, HelpRequestId, HelperProfile, TelegramUserId};

#[derive(Debug, Error)]
pub enum AppError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        Self::Storage(err.to_string())
    }
}

/// Authoritative request storage. All state transitions are short atomic
/// writes; no Telegram/network I/O happens inside them.
#[async_trait]
pub trait RequestRepository: Send + Sync {
    /// Inserts a request row (draft or open). Caller provides a valid domain object.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when the underlying storage operation fails.
    async fn insert(&self, request: &HelpRequest) -> Result<(), AppError>;

    /// Publishes a draft (`Draft -> Open`).
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when the underlying storage operation fails.
    async fn publish(&self, request_id: HelpRequestId) -> Result<(), AppError>;

    /// Attempts an atomic accept of an open request.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when the underlying storage operation fails.
    async fn accept(
        &self,
        request_id: HelpRequestId,
        helper_id: TelegramUserId,
    ) -> Result<AcceptOutcome, AppError>;

    /// Cancels a draft/open/matched request as the requester.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when the underlying storage operation fails.
    async fn cancel(
        &self,
        request_id: HelpRequestId,
        requester_id: TelegramUserId,
    ) -> Result<(), AppError>;

    /// Expires open requests whose TTL elapsed (`Open -> Expired`) and marks
    /// their pending dispatches expired.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when the underlying storage operation fails.
    async fn expire_due(&self, now_unix: i64) -> Result<u64, AppError>;

    /// Confirms a matched request as the requester (`Matched -> Resolved`).
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when the underlying storage operation fails.
    async fn resolve(
        &self,
        request_id: HelpRequestId,
        requester_id: TelegramUserId,
    ) -> Result<(), AppError>;

    /// Helper withdraws from a match (`Matched -> Open`). Creates a new request
    /// version; old capabilities MUST NOT be resurrected (caller fans out a new
    /// dispatch wave).
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when the underlying storage operation fails.
    async fn withdraw(
        &self,
        request_id: HelpRequestId,
        helper_id: TelegramUserId,
    ) -> Result<(), AppError>;
}

/// Helper settings storage (coarse areas only, no tracking).
#[async_trait]
pub trait HelperRepository: Send + Sync {
    /// Ensures a `users` row exists for the Telegram identity.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when the underlying storage operation fails.
    async fn ensure_user(
        &self,
        user_id: TelegramUserId,
        consent_version: &str,
        now_unix: i64,
    ) -> Result<(), AppError>;

    /// Replaces helper settings (enabled flag, areas, kinds).
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when the underlying storage operation fails.
    async fn upsert_helper(&self, profile: &HelperProfile) -> Result<(), AppError>;
}

/// Fan-out storage. Projection ids (`view_id`, `chat_id`, …) are adapter
/// state, never domain truth.
#[async_trait]
pub trait DispatchRepository: Send + Sync {
    /// Creates one `pending` dispatch per helper.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when the underlying storage operation fails.
    async fn fanout(
        &self,
        request_id: HelpRequestId,
        helper_ids: &[TelegramUserId],
    ) -> Result<(), AppError>;
}

pub struct BikeAidService<R> {
    requests: R,
}

impl<R> BikeAidService<R>
where
    R: RequestRepository,
{
    pub const fn new(requests: R) -> Self {
        Self { requests }
    }

    /// Delegates to the repository atomic accept.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when the underlying storage operation fails.
    pub async fn accept_request(
        &self,
        request_id: HelpRequestId,
        helper_id: TelegramUserId,
    ) -> Result<AcceptOutcome, AppError> {
        self.requests.accept(request_id, helper_id).await
    }
}
