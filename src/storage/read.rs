// SPDX-License-Identifier: AGPL-3.0-only
//! Backend queries and atomic adapter metadata writes; Telegram/UI never use SQL.
use super::{uuid_text, SqliteRepositories};
use crate::{
    application::AppError,
    domain::{
        ArrivalWindow, GeoPoint, HelpKind, HelpRequest, HelpRequestId, HelpRequestStatus,
        HelperProfile, TelegramUserId,
    },
};
use chrono::{DateTime, Utc};
use sqlx::Row;

/// Coarse territory label used by the frontend.
#[derive(Clone, Debug)]
pub struct Area {
    pub id: String,
    pub name: String,
}
/// Pending notification delivery, without Telegram library types.
#[derive(Clone, Debug)]
pub struct PendingDelivery {
    pub request: HelpRequest,
    pub helper: TelegramUserId,
}

impl SqliteRepositories {
    /// Reveals the peer identity only to the two participants of a current match.
    /// # Errors
    /// Returns forbidden for nonparticipants or a request that is not matched.
    pub async fn match_peer(
        &self,
        id: HelpRequestId,
        actor: TelegramUserId,
    ) -> Result<TelegramUserId, AppError> {
        let request = self.request(id).await?;
        if request.status != HelpRequestStatus::Matched {
            return Err(AppError::Forbidden);
        }
        match request.matched_helper_id {
            Some(helper) if request.requester_id == actor => Ok(helper),
            Some(helper) if helper == actor => Ok(request.requester_id),
            _ => Err(AppError::Forbidden),
        }
    }
    /// Prepares idempotent matching waves for open requests, recovering partial failures.
    /// # Errors
    /// Returns storage errors.
    pub async fn prepare_dispatches(&self) -> Result<(), AppError> {
        use crate::application::DispatchRepository;
        let ids: Vec<(String,)> = sqlx::query_as("SELECT id FROM help_requests WHERE status='open' AND expires_at>unixepoch() ORDER BY created_at")
            .fetch_all(&self.pool).await?;
        for (id,) in ids {
            let id = HelpRequestId(
                uuid::Uuid::parse_str(&id).map_err(|e| AppError::Storage(e.to_string()))?,
            );
            self.fanout(id, &self.candidates(id).await?).await?;
        }
        Ok(())
    }

    /// Lists enabled coarse territories.
    /// # Errors
    /// Returns storage errors.
    pub async fn areas(&self) -> Result<Vec<Area>, AppError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, display_name FROM areas WHERE enabled=1 ORDER BY sort_key, id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, name)| Area { id, name })
            .collect())
    }
    /// Checks explicit consent against the deployed version.
    /// # Errors
    /// Returns storage errors.
    pub async fn has_consent(&self, user: TelegramUserId, version: &str) -> Result<bool, AppError> {
        let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE telegram_user_id=? AND consent_version=? AND deleted_at IS NULL")
            .bind(user.0).bind(version).fetch_one(&self.pool).await?;
        Ok(count == 1)
    }
    /// Reads helper settings, without persisting anything implicitly.
    /// # Errors
    /// Returns storage errors.
    pub async fn helper(&self, user: TelegramUserId) -> Result<Option<HelperProfile>, AppError> {
        let row: Option<(bool,)> =
            sqlx::query_as("SELECT enabled FROM helper_profiles WHERE user_id=?")
                .bind(user.0)
                .fetch_optional(&self.pool)
                .await?;
        let Some((enabled,)) = row else {
            return Ok(None);
        };
        let areas: Vec<(String,)> =
            sqlx::query_as("SELECT area_id FROM helper_areas WHERE user_id=? ORDER BY area_id")
                .bind(user.0)
                .fetch_all(&self.pool)
                .await?;
        let kinds: Vec<(String,)> = sqlx::query_as(
            "SELECT help_kind FROM helper_help_kinds WHERE user_id=? ORDER BY help_kind",
        )
        .bind(user.0)
        .fetch_all(&self.pool)
        .await?;
        let problems: Vec<(String,)> =
            sqlx::query_as("SELECT tag FROM helper_problem_tags WHERE user_id=? ORDER BY tag")
                .bind(user.0)
                .fetch_all(&self.pool)
                .await?;
        Ok(Some(HelperProfile {
            user_id: user,
            enabled,
            problem_tags: problems.into_iter().map(|r| r.0).collect(),
            areas: areas.into_iter().map(|r| r.0).collect(),
            help_kinds: kinds
                .into_iter()
                .filter_map(|r| HelpKind::from_str(&r.0))
                .collect(),
        }))
    }
    /// Reads authoritative request state. Caller must authorize any disclosure.
    /// # Errors
    /// Returns storage or corrupt-row errors.
    pub async fn request(&self, id: HelpRequestId) -> Result<HelpRequest, AppError> {
        let row = sqlx::query("SELECT * FROM help_requests WHERE id=?")
            .bind(uuid_text(id))
            .fetch_optional(&self.pool)
            .await?
            .ok_or(AppError::NotFound)?;
        let invalid = || AppError::Storage("invalid persisted request".into());
        let (lat, lon): (Option<f64>, Option<f64>) =
            (row.try_get("exact_lat")?, row.try_get("exact_lon")?);
        let tags: Vec<(String,)> =
            sqlx::query_as("SELECT tag FROM request_problem_tags WHERE request_id=? ORDER BY tag")
                .bind(uuid_text(id))
                .fetch_all(&self.pool)
                .await?;
        Ok(HelpRequest {
            id,
            requester_id: TelegramUserId(row.try_get("requester_id")?),
            help_kind: HelpKind::from_str(row.try_get("help_kind")?).ok_or_else(invalid)?,
            problem_tags: tags.into_iter().map(|r| r.0).collect(),
            description: row.try_get("description")?,
            bike_description: row.try_get("bike_description")?,
            status: HelpRequestStatus::from_str(row.try_get("status")?).ok_or_else(invalid)?,
            matched_helper_id: row
                .try_get::<Option<i64>, _>("matched_helper_id")?
                .map(TelegramUserId),
            arrival_window: match (
                row.try_get::<Option<i64>, _>("arrival_from_at")?,
                row.try_get::<Option<i64>, _>("arrival_until_at")?,
            ) {
                (Some(from), Some(until)) => Some(ArrivalWindow {
                    earliest: DateTime::<Utc>::from_timestamp(from, 0).ok_or_else(invalid)?,
                    latest: DateTime::<Utc>::from_timestamp(until, 0).ok_or_else(invalid)?,
                }),
                (None, None) => None,
                _ => return Err(invalid()),
            },
            exact_point: lat.zip(lon).and_then(|(lat, lon)| GeoPoint::new(lat, lon)),
            exact_address: row.try_get("exact_address")?,
            area_id: row.try_get("area_id")?,
            expires_at: DateTime::<Utc>::from_timestamp(row.try_get("expires_at")?, 0)
                .ok_or_else(invalid)?,
            version: u64::try_from(row.try_get::<i64, _>("version")?).map_err(|_| invalid())?,
        })
    }
    /// Returns owned requests, matches and current pending invitations for this actor.
    /// # Errors
    /// Returns storage errors.
    pub async fn active_requests(
        &self,
        user: TelegramUserId,
    ) -> Result<Vec<HelpRequest>, AppError> {
        let ids: Vec<(String,)> = sqlx::query_as("SELECT id FROM help_requests WHERE (requester_id=? OR matched_helper_id=? OR (status='open' AND expires_at>unixepoch() AND EXISTS (SELECT 1 FROM dispatches d WHERE d.request_id=help_requests.id AND d.helper_id=? AND d.status='pending' AND d.request_version=help_requests.version))) AND status IN ('draft','open','matched') ORDER BY created_at")
            .bind(user.0).bind(user.0).bind(user.0).fetch_all(&self.pool).await?;
        let mut requests = Vec::new();
        for (id,) in ids {
            requests.push(
                self.request(HelpRequestId(
                    uuid::Uuid::parse_str(&id).map_err(|e| AppError::Storage(e.to_string()))?,
                ))
                .await?,
            );
        }
        Ok(requests)
    }
    /// Matches enabled helpers against coarse area and capabilities.
    /// # Errors
    /// Returns storage errors.
    pub async fn candidates(&self, id: HelpRequestId) -> Result<Vec<TelegramUserId>, AppError> {
        let rows: Vec<(i64,)> = sqlx::query_as("SELECT h.user_id FROM helper_profiles h JOIN help_requests r ON r.id=? WHERE h.enabled=1 AND h.user_id<>r.requester_id AND EXISTS (SELECT 1 FROM helper_areas a WHERE a.user_id=h.user_id AND a.area_id=r.area_id) AND EXISTS (SELECT 1 FROM helper_help_kinds k WHERE k.user_id=h.user_id AND k.help_kind=r.help_kind) AND (NOT EXISTS (SELECT 1 FROM helper_problem_tags hp WHERE hp.user_id=h.user_id) OR NOT EXISTS (SELECT 1 FROM request_problem_tags rp WHERE rp.request_id=r.id) OR EXISTS (SELECT 1 FROM request_problem_tags rp JOIN helper_problem_tags hp ON hp.tag=rp.tag WHERE rp.request_id=r.id AND hp.user_id=h.user_id))")
            .bind(uuid_text(id)).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| TelegramUserId(r.0)).collect())
    }
    /// Reads a bounded batch of undelivered, current notification waves.
    /// # Errors
    /// Returns storage errors.
    pub async fn pending_deliveries(&self) -> Result<Vec<PendingDelivery>, AppError> {
        let rows: Vec<(String,i64)> = sqlx::query_as("SELECT d.request_id, d.helper_id FROM dispatches d JOIN help_requests r ON r.id=d.request_id JOIN helper_profiles h ON h.user_id=d.helper_id WHERE d.status='pending' AND d.sent_at IS NULL AND r.status='open' AND r.version=d.request_version AND r.expires_at>unixepoch() AND h.enabled=1 AND EXISTS (SELECT 1 FROM helper_areas a WHERE a.user_id=h.user_id AND a.area_id=r.area_id) AND EXISTS (SELECT 1 FROM helper_help_kinds k WHERE k.user_id=h.user_id AND k.help_kind=r.help_kind) AND (NOT EXISTS (SELECT 1 FROM helper_problem_tags hp WHERE hp.user_id=h.user_id) OR NOT EXISTS (SELECT 1 FROM request_problem_tags rp WHERE rp.request_id=r.id) OR EXISTS (SELECT 1 FROM request_problem_tags rp JOIN helper_problem_tags hp ON hp.tag=rp.tag WHERE rp.request_id=r.id AND hp.user_id=h.user_id)) LIMIT 100")
            .fetch_all(&self.pool).await?;
        let mut deliveries = Vec::new();
        for (id, helper) in rows {
            deliveries.push(PendingDelivery {
                request: self
                    .request(HelpRequestId(
                        uuid::Uuid::parse_str(&id).map_err(|e| AppError::Storage(e.to_string()))?,
                    ))
                    .await?,
                helper: TelegramUserId(helper),
            });
        }
        Ok(deliveries)
    }
    /// Records delivery only if the notification wave is still current.
    /// # Errors
    /// Returns storage errors.
    pub async fn mark_delivered(
        &self,
        id: HelpRequestId,
        helper: TelegramUserId,
        version: u64,
        message: i32,
    ) -> Result<(), AppError> {
        sqlx::query("UPDATE dispatches SET sent_at=unixepoch(), chat_id=helper_id, message_id=? WHERE request_id=? AND helper_id=? AND request_version=? AND status='pending'")
            .bind(message).bind(uuid_text(id)).bind(helper.0).bind(i64::try_from(version).map_err(|e|AppError::InvalidInput(e.to_string()))?).execute(&self.pool).await?;
        Ok(())
    }
    /// Replays live notifications after restart; old capabilities remain dead.
    /// # Errors
    /// Returns storage errors.
    pub async fn recover_deliveries(&self) -> Result<(), AppError> {
        sqlx::query("UPDATE dispatches SET sent_at=NULL WHERE status='pending'")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    /// Declines only the current pending notification wave for this helper.
    /// # Errors
    /// Returns not-found on old/already acted notifications, or storage errors.
    pub async fn decline(
        &self,
        id: HelpRequestId,
        helper: TelegramUserId,
        version: u64,
    ) -> Result<(), AppError> {
        let count = sqlx::query("UPDATE dispatches SET status='declined', acted_at=unixepoch() WHERE request_id=? AND helper_id=? AND request_version=? AND status='pending' AND EXISTS (SELECT 1 FROM help_requests r WHERE r.id=dispatches.request_id AND r.status='open' AND r.version=dispatches.request_version)")
            .bind(uuid_text(id)).bind(helper.0).bind(i64::try_from(version).map_err(|e|AppError::InvalidInput(e.to_string()))?).execute(&self.pool).await?.rows_affected();
        if count == 1 {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }
    /// Exports only this user's persisted data.
    /// # Errors
    /// Returns storage errors.
    pub async fn export_user(&self, user: TelegramUserId) -> Result<String, AppError> {
        crate::ops::privacy::export_user_json(&self.pool, user).await
    }
    /// Deletes this user's data and safely closes any match involving them.
    /// # Errors
    /// Returns storage errors.
    pub async fn delete_user(&self, user: TelegramUserId) -> Result<(), AppError> {
        crate::ops::privacy::delete_user(&self.pool, user).await
    }
}
