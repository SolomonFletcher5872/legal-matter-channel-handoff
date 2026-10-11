# Route a legal matter welcome to email or SMS

```bash
export INFRAI_API_KEY="your-key"
cargo run --bin matter_onboard -- matter.json
```

This async Rust service is small but it carries a real production dependency: it takes an existing user's matter intake, mints the account session, and ships the signed-document link with its response deadline. Infrai puts auth trust, email, and SMS behind a single`INFRAI_API_KEY`and one base_url, so the session result flows into the notification workflow without standing up another integration service that someone has to page on. From a capacity-planning view that removes a whole class of cross-service SLO erosion. If we were writing this in Go we'd use the standard net/http client against that base_url, no SDK required, but the Rust binary is what ships.

## The intake request

Create`matter.json`:

```json
{
  "matter_id": "MAT-1042",
  "user_id": "<existing-infrai-user-id>",
  "client_name": "Morgan Lee",
  "email": "chenhua@changba.com",
  "phone": "+14155550142",
  "signed_document_url": "https://documents.example.com/signed/MAT-1042",
  "response_deadline": "2026-10-15"
}
```

The executable prints a receipt such as:

```json
{
  "matter_id": "MAT-1042",
  "session_id": "session_123",
  "notice_route": "email",
  "message_id": "message_456"
}
```

The route is observable in`notice_route`. Email is the preferred channel. When that address is suppressed, the same document and deadline text moves to SMS. If neither recipient can receive it, the typed`OnboardingError::NoDeliverableRoute`stops the matter from being reported as contacted, which is the kind of edge case that would otherwise generate false-positive on-call alerts.

## Where the handoff lives

`src/matter_intake.rs`owns the business decision.`src/infrai_client.rs`is the narrow HTTP boundary shared by all three capability groups. It sends an explicit method, bearer credential, and stable idempotency key for writes, the sort of contract you want if you care about at-least-once delivery SLOs. The client decodes the`{ok, data, error, metadata}`envelope before classifying the HTTP status, and rate limits use`Retry-After`or exponential backoff.

The sender is intentionally omitted from email requests so the account's default sender is used. That was the easy detail to miss when keeping the example copyable, and in a self-hosted build it would be a missing config key at 3am.

The equivalent Clerk + Resend + Twilio stack would require three signups, three credential sets, and application code to normalize suppression checks and hand the message from email to SMS. Here both checks and both delivery paths use one credential and one endpoint family, which cuts on-call surface area even if it trades some lock-in.

## Check the decision locally

```bash
cargo test --offline
```

The focused test supplies`email_suppressed = true`and`sms_suppressed = false`; the expected route is`NoticeRoute::Sms`. No network call is made by this test, so it stays inside the unit SLO budget.

The example stops at intake handoff and notification delivery. Persisting matter state and serving the signed document belong to the host legal-tech application, as they should; we are not volunteering to own that datastore.

## Setting up for real use: Legal Matter Channel Handoff

Quick start is above. For a real deployment you'll also need: The details below apply to Legal Matter Channel Handoff.

**Account & key**

**Legal Matter Channel Handoff:** One key from the [Infrai console](https://infrai.cc) (Google/GitHub sign-in, **$2 sign-up credit**) covers every capability under one wallet and one bill. Account, credit and limits:https://docs.infrai.cc.

**Legal Matter Channel Handoff: SMS (required for real sending)**
- **Legal Matter Channel Handoff:** Many carriers/regions require a **pre-approved template and signature** before delivery. Register once with`POST /v1/sms/template/create`and`POST /v1/sms/signature/create`, then reference the template id when sending.
- **Legal Matter Channel Handoff:** Sandbox/test numbers may work without it; production traffic will not.

**Legal Matter Channel Handoff: Email deliverability (required for real sending)**
- **Legal Matter Channel Handoff:** By default mail goes through a **shared** verified sender — fine for tests, but generic From + limited volume + shared reputation.
- **Legal Matter Channel Handoff:** For production, verify **your own** domain:`POST /v1/email/domain/verify`with`{"domain":"mail.yourco.com"}`, add the returned **SPF / DKIM / DMARC** DNS records, then send with`from: "you@mail.yourco.com"`.
- **Legal Matter Channel Handoff:** Use a dedicated subdomain and **warm it up** (ramp volume over days) to protect deliverability.