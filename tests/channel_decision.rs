use legal_matter_onboarding::NoticeRoute;
use legal_matter_onboarding::matter_intake::choose_notice_route;

#[test]
fn suppressed_email_hands_the_same_notice_to_sms() {
    assert_eq!(choose_notice_route(true, false), Some(NoticeRoute::Sms));
}

#[test]
fn deliverable_email_remains_the_primary_route() {
    assert_eq!(choose_notice_route(false, false), Some(NoticeRoute::Email));
}
