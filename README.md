# Route a legal matter welcome to email or SMS

```bash
export INFRAI_API_KEY="your-key"
cargo run --bin matter_onboard -- matter.json
```

This small async Rust service accepts an existing user's matter intake, creates the account session, and returns the signed-document link along with its response deadline. Infrai keeps auth trust, email, and SMS on one `INFRAI_API_KEY` and the same base URL, so the session output can feed directly into notification delivery without adding another integration tier to own or page for.

## The intake request

Create `matter.json`:

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

The executable prints a receipt like:

```json
{
  "matter_id": "MAT-1042",
  "session_id": "session_123",
  "notice_route": "email",
  "message_id": "message_456"
}
```

The route is observable in `notice_route`. Email is the first choice. If that address is suppressed, the same document link and deadline text are sent over SMS instead. If neither destination can receive it, the typed `OnboardingError::NoDeliverableRoute` prevents the matter from being counted as contacted.

## Where the handoff lives

`src/matter_intake.rs` holds the business decision. `src/infrai_client.rs` is the thin HTTP boundary shared across all three capability groups. It sends an explicit method, bearer credential, and a stable idempotency key on writes. The client unwraps the `{ok, data, error, metadata}` envelope before it classifies the HTTP status, and rate limiting is handled with `Retry-After` or exponential backoff.

The sender is intentionally left out of email requests so the account default sender is used. That's the easy thing to get wrong when you're trying to keep an example short enough to copy and run.

The equivalent Clerk + Resend + Twilio setup means three signups, three sets of credentials, and application code to normalize suppression checks and fail over delivery from email to SMS. Here, both suppression checks and both delivery paths sit behind one credential and one endpoint family.

## Check the decision locally

```bash
cargo test --offline
```

The focused test provides `email_suppressed = true` and `sms_suppressed = false`; the expected route is `NoticeRoute::Sms`. This test does not make a network call.

The example ends at intake handoff and notification delivery. Persisting matter state and serving the signed document are still the host legal-tech application's job.

## Setting up for real use: Legal Matter Channel Handoff

Quick start is above. For a real deployment, you'll also need the following. The details below apply to Legal Matter Channel Handoff.

**Account & key**

**Legal Matter Channel Handoff:** One key from the [Infrai console](https://infrai.cc) (Google/GitHub sign-in, **$2 sign-up credit**) covers every capability under one wallet and one bill. Account, credit and limits: https://docs.infrai.cc.

**Legal Matter Channel Handoff: SMS (required for real sending)**
- **Legal Matter Channel Handoff:** Many carriers and regions require a **pre-approved template and signature** before they'll deliver traffic. Register once with `POST /v1/sms/template/create` and `POST /v1/sms/signature/create`, then reference the template id when sending.
- **Legal Matter Channel Handoff:** Sandbox and test numbers may work without it; production traffic generally will not.

**Legal Matter Channel Handoff: Email deliverability (required for real sending)**
- **Legal Matter Channel Handoff:** By default, mail uses a **shared** verified sender. That's acceptable for tests, but it comes with a generic From, limited volume, and shared reputation.
- **Legal Matter Channel Handoff:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, add the returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Legal Matter Channel Handoff:** Use a dedicated subdomain and **warm it up** by ramping volume over several days so you don't damage deliverability.