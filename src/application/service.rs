// SPDX-License-Identifier: AGPL-3.0-only

use async_trait::async_trait;
use thiserror::Error;

use crate::domain::{AcceptOutcome, HelpRequestId, TelegramUserId};

#[derive(Debug, Error)]
pub enum AppError {
    #[error("storage error: {0}")]
    Storage(String),
}

#[async_trait]
pub trait RequestRepository: Send + Sync {
    async fn accept(
        &self,
        request_id: HelpRequestId,
        helper_id: TelegramUserId,
    ) -> Result<AcceptOutcome, AppError>;
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

    pub async fn accept_request(
        &self,
        request_id: HelpRequestId,
        helper_id: TelegramUserId,
    ) -> Result<AcceptOutcome, AppError> {
        self.requests.accept(request_id, helper_id).await
    }
}
