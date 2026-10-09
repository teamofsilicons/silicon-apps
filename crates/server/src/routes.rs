//! Every public HTTP route, as `(method, path template)`.
//!
//! This table is the router's contract. The `/v1` dispatcher refuses any
//! request whose method and path do not match an entry here before it reads
//! credentials or contacts another service, and `openapi.json` must describe
//! exactly these operations (a test compares them).

pub const ROUTES: &[(&str, &str)] = &[
    ("GET", "/health"),
    ("GET", "/openapi.json"),
    ("GET", "/.well-known/agent.json"),
    ("GET", "/.well-known/agent-card.json"),
    ("GET", "/.well-known/silicon-apps-keys.json"),
    ("GET", "/v1/openapi.json"),
    ("GET", "/v1/capabilities"),
    ("GET", "/v1/session"),
    ("GET", "/v1/me"),
    ("GET", "/v1/targets"),
    ("GET", "/v1/apps"),
    ("POST", "/v1/apps"),
    ("GET", "/v1/apps/availability/{app_id}"),
    ("GET", "/v1/apps/{app_id}"),
    ("PATCH", "/v1/apps/{app_id}"),
    ("PUT", "/v1/apps/{app_id}/access"),
    ("GET", "/v1/apps/{app_id}/readiness"),
    ("POST", "/v1/apps/{app_id}/publish"),
    ("POST", "/v1/apps/{app_id}/secret/rotate"),
    ("GET", "/v1/apps/{app_id}/authors"),
    ("POST", "/v1/apps/{app_id}/authors/leave"),
    ("DELETE", "/v1/apps/{app_id}/authors/{uuid}"),
    ("POST", "/v1/apps/{app_id}/admin"),
    ("GET", "/v1/apps/{app_id}/invites"),
    ("POST", "/v1/apps/{app_id}/invites"),
    ("DELETE", "/v1/apps/{app_id}/invites/{invite_id}"),
    ("GET", "/v1/invites"),
    ("POST", "/v1/invites/{invite_id}/accept"),
    ("POST", "/v1/invites/{invite_id}/decline"),
    ("GET", "/v1/apps/{app_id}/history"),
    ("GET", "/v1/apps/{app_id}/packages"),
    ("POST", "/v1/apps/{app_id}/packages/{target}"),
    ("GET", "/v1/apps/{app_id}/packages/{package_id}/download"),
    ("GET", "/v1/apps/{app_id}/releases"),
    ("POST", "/v1/apps/{app_id}/releases"),
    ("POST", "/v1/apps/{app_id}/releases/{release_id}/promote"),
    ("POST", "/v1/apps/{app_id}/releases/{release_id}/withdraw"),
    ("GET", "/v1/apps/{app_id}/resolve"),
    ("POST", "/v1/apps/{app_id}/installs"),
    ("GET", "/v1/apps/{app_id}/reviews"),
    ("PUT", "/v1/apps/{app_id}/review"),
    ("DELETE", "/v1/apps/{app_id}/review"),
    ("GET", "/v1/apps/{app_id}/webhook"),
    ("PUT", "/v1/apps/{app_id}/webhook"),
    ("POST", "/v1/apps/{app_id}/webhook/rotate"),
    ("POST", "/v1/apps/{app_id}/media"),
    ("GET", "/v1/apps/{app_id}/media/{media_id}"),
    ("GET", "/v1/apps/{app_id}/events"),
    ("GET", "/v1/apps/{app_id}/events/stream"),
    ("GET", "/v1/authors/{uuid}"),
    ("GET", "/v1/events"),
    ("GET", "/v1/events/stream"),
    ("GET", "/v1/subscriptions"),
    ("POST", "/v1/subscriptions"),
    ("GET", "/v1/subscriptions/{subscription_id}"),
    ("PATCH", "/v1/subscriptions/{subscription_id}"),
    ("DELETE", "/v1/subscriptions/{subscription_id}"),
    ("GET", "/v1/subscriptions/{subscription_id}/deliveries"),
    ("POST", "/v1/subscriptions/{subscription_id}/secret/rotate"),
    ("POST", "/v1/subscriptions/{subscription_id}/ping"),
    ("GET", "/v1/keys"),
    ("POST", "/v1/keys"),
    ("DELETE", "/v1/keys/{key_id}"),
    ("POST", "/v1/platforms"),
    ("POST", "/v1/reports"),
    ("POST", "/v1/telemetry"),
    ("GET", "/v1/auth/login"),
    ("GET", "/v1/auth/callback"),
    ("POST", "/v1/auth/exchange"),
    ("POST", "/v1/auth/refresh"),
    ("POST", "/v1/auth/logout"),
];

/// The matching template for a concrete request path. Literal segments must be
/// equal; `{name}` matches one non-empty segment. Earlier entries win, so a
/// literal such as `availability` is chosen before an `{app_id}` placeholder.
pub fn find(method: &str, path: &str) -> Option<&'static str> {
    let segments: Vec<_> = path.trim_start_matches('/').split('/').collect();
    ROUTES
        .iter()
        .filter(|(m, _)| *m == method)
        .map(|(_, template)| *template)
        .find(|template| {
            let parts: Vec<_> = template.trim_start_matches('/').split('/').collect();
            parts.len() == segments.len()
                && parts.iter().zip(&segments).all(|(part, segment)| {
                    if part.starts_with('{') {
                        !segment.is_empty()
                    } else {
                        part == segment
                    }
                })
        })
}
