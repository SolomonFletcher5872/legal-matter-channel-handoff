use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ApiError, InfraiClient};

#[derive(Debug, Clone, Deserialize)]
pub struct MatterIntake {
    pub matter_id: String,
    pub user_id: String,
    pub client_name: String,
    pub email: String,
    pub phone: String,
    pub signed_document_url: String,
    pub response_deadline: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeRoute {
    Email,
    Sms,
}

#[derive(Debug, Serialize)]
pub struct OnboardingReceipt {
    pub matter_id: String,
    pub session_id: String,
    pub notice_route: NoticeRoute,
    pub message_id: String,
}

#[derive(Debug, Error)]
pub enum OnboardingError {
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error("both contact routes are suppressed for matter {matter_id}")]
    NoDeliverableRoute { matter_id: String },
}

pub fn choose_notice_route(email_suppressed: bool, sms_suppressed: bool) -> Option<NoticeRoute> {
    match (email_suppressed, sms_suppressed) {
        (false, _) => Some(NoticeRoute::Email),
        (true, false) => Some(NoticeRoute::Sms),
        (true, true) => None,
    }
}

pub async fn onboard_matter(
    infrai: &InfraiClient,
    intake: MatterIntake,
) -> Result<OnboardingReceipt, OnboardingError> {
    let session = infrai
        .create_session(
            &intake.user_id,
            &format!("matter:{}:session", intake.matter_id),
        )
        .await?;

    let email_suppressed = infrai.email_is_suppressed(&intake.email).await?;
    let sms_suppressed = if email_suppressed {
        infrai.sms_is_suppressed(&intake.phone).await?
    } else {
        false
    };
    let route = choose_notice_route(email_suppressed, sms_suppressed).ok_or_else(|| {
        OnboardingError::NoDeliverableRoute {
            matter_id: intake.matter_id.clone(),
        }
    })?;

    let notice = format!(
        "Hello {}, your signed matter document is ready: {}. Please respond by {}.",
        intake.client_name, intake.signed_document_url, intake.response_deadline
    );
    let idempotency_key = format!("matter:{}:welcome", intake.matter_id);
    let message_id = match route {
        NoticeRoute::Email => {
            infrai
                .send_email(
                    &intake.email,
                    &format!("Matter {} is ready", intake.matter_id),
                    &notice,
                    &idempotency_key,
                )
                .await?
                .message_id
        }
        NoticeRoute::Sms => {
            infrai
                .send_sms(&intake.phone, &notice, &idempotency_key)
                .await?
                .message_id
        }
    };

    Ok(OnboardingReceipt {
        matter_id: intake.matter_id,
        session_id: session.session_id,
        notice_route: route,
        message_id,
    })
}
