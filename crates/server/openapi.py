#!/usr/bin/env python3
"""Write crates/server/openapi.json, the OpenAPI 3.1 description the API serves.

Edit this file, not the JSON, then from the repository root run:

    python3 crates/server/openapi.py crates/server/openapi.json
    npx @redocly/cli lint crates/server/openapi.json

The discovery tests check that the document and the router's route table
describe exactly the same operations.
"""
import json, sys

def ref(name): return {"$ref": f"#/components/schemas/{name}"}
def resp(name): return {"$ref": f"#/components/responses/{name}"}
def param(name): return {"$ref": f"#/components/parameters/{name}"}
def arr(items): return {"type": "array", "items": items}
S = {"type": "string"}
I = {"type": "integer"}
B = {"type": "boolean"}
def nullable(t): return {"type": [t["type"], "null"]} if "type" in t and isinstance(t["type"], str) and len(t) == 1 else {"anyOf": [t, {"type": "null"}]}
def obj(props, required=None, extra=None, desc=None):
    o = {"type": "object", "properties": props}
    if required: o["required"] = required
    if extra is not None: o["additionalProperties"] = extra
    if desc: o["description"] = desc
    return o
def json_body(schema, desc=None, required=True):
    b = {"required": required, "content": {"application/json": {"schema": schema}}}
    if desc: b["description"] = desc
    return b
def ok(schema, desc="OK", headers=None):
    r = {"description": desc, "content": {"application/json": {"schema": schema}}}
    if headers: r["headers"] = headers
    return r

TARGETS = ["linux-x86_64","linux-i686","linux-aarch64","linux-armv7hf","windows-x86_64","windows-i686","windows-aarch64","macos-x86_64","macos-aarch64"]
EVENT_TYPES = ["app.created","app.imported_from_accounts","app.id_migrated","app.details_changed","app.access_changed","app.published","app.secret_rotated","app.installed","media.uploaded","accounts.webhook_changed","package.validation_started","package.validation_step","package.accepted","package.validation_failed","release.created","release.promoted","release.withdrawn","author.invited","author.joined","author.invite_declined","author.invite_cancelled","author.left","author.removed","author.admin_transferred","review.updated","review.removed","ping"]

schemas = {
 "Error": obj({"error": obj({
     "code": {"type": "string", "description": "Stable machine-readable code, for example `not_found`, `rate_limited` or `unsupported_api_version`."},
     "message": {"type": "string", "description": "What went wrong, in a sentence."},
     "hint": {"type": "string", "description": "What to do next."},
     "details": {"description": "Extra structured data, or null."}},
     ["code","message","hint","details"])}, ["error"]),
 "Health": obj({"status": S, "service": S, "version": S}, ["status","service","version"]),
 "Identity": obj({"uuid": {"type":"string","description":"Permanent Silicon Accounts UUID."}, "id": {"type":"string","description":"Current public ID, c:name or si:name."}, "display_name": S, "verified_emails": arr(S)}, ["uuid","id"]),
 "Session": obj({"authenticated": B, "account": {"anyOf":[ref("Identity"),{"type":"null"}]}}, ["authenticated","account"]),
 "Author": obj({"uuid": S, "id": S, "display_name": S, "joined_at": {"type":"string","format":"date-time"}}, ["uuid","id","display_name","joined_at"]),
 "ValidationCheck": obj({"command": {"type":"string","enum":["--help","accounts --json","login status --json"]}, "exit_code": {"type":["integer","null"]}, "stdout": S, "stderr": S, "passed": B, "expected": S}, ["command","exit_code","stdout","stderr","passed","expected"]),
 "Package": obj({"id": S, "target": {"type":"string","enum":TARGETS}, "sha256": {"type":"string","pattern":"^[0-9a-f]{64}$"}, "size": I, "command": S, "validation": arr(ref("ValidationCheck")), "created_at": {"type":"string","format":"date-time"}, "install_script": {"anyOf":[ref("InstallScript"),{"type":"null"}],"description":"The install script this target runs, or null when it has none."}, "inspected": {"type":"boolean","description":"Whether Apps has read the package's install script information. Always true for new uploads."}, "author_signature": {"anyOf":[ref("AuthorSignature"),{"type":"null"}],"description":"The uploading author's own signature, when they signed the package."}}, ["id","target","sha256","size","command","validation","created_at"]),
 "Release": obj({"id": S, "app_id": S, "channel": {"type":"string","enum":["development","production"]}, "version": {"type":"string","pattern":"^(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)$"}, "package_ids": arr(S), "notes": S, "created_at": {"type":"string","format":"date-time"}, "promoted_from": {"type":["string","null"],"description":"The development release this production release was promoted from."}, "signatures": {"type":"object","additionalProperties":ref("ReleaseSignature"),"description":"The API's Ed25519 signature over each package's release manifest, by package ID. Every package Apps serves is signed."}, "signed_by_author": {"type":"boolean","description":"Every package in this release also carries a valid author signature."}, "withdrawn": {"anyOf":[ref("Withdrawal"),{"type":"null"}],"description":"Set when an author withdrew the release. A withdrawn release is never served."}}, ["id","app_id","channel","version","package_ids","notes","created_at"]),
 "ReleaseSignature": obj({"key_id": {"type":"string","description":"The signing key, listed at /.well-known/silicon-apps-keys.json."}, "algorithm": {"type":"string","const":"ed25519"}, "signature": {"type":"string","description":"Base64 Ed25519 signature over the release manifest message."}, "install_script_sha256": {"type":["string","null"]}, "signed_at": {"type":"string","format":"date-time"}}, ["key_id","algorithm","signature","install_script_sha256","signed_at"]),
 "Withdrawal": obj({"at": {"type":"string","format":"date-time"}, "by_uuid": S, "by_id": S, "reason": {"type":"string","maxLength":500}}, ["at","by_uuid","by_id","reason"]),
 "WithdrawnRelease": obj({"release_id": S, "version": S, "channel": {"type":"string","enum":["development","production"]}, "reason": S, "withdrawn_at": {"type":"string","format":"date-time"}, "withdrawn_by": S}, ["release_id","version","channel","reason","withdrawn_at","withdrawn_by"]),
 "InstallScript": obj({"path": {"type":"string","description":"Path inside the package, as written in apps.yaml."}, "sha256": {"type":"string","pattern":"^[0-9a-f]{64}$"}, "size": I}, ["path","sha256","size"]),
 "AuthorSignature": obj({"key_id": {"type":"string","description":"`ak_` and 16 hex characters."}, "algorithm": {"type":"string","const":"ed25519"}, "public_key": {"type":"string","description":"Base64 Ed25519 public key."}, "signature": {"type":"string","description":"Base64 signature over the author package message."}, "signer_uuid": S, "signer_id": S, "signed_at": {"type":"string","format":"date-time"}}, ["key_id","algorithm","public_key","signature","signer_uuid","signer_id","signed_at"]),
 "SignedManifest": obj({"app_id": S, "target": {"type":"string","enum":TARGETS}, "version": S, "channel": {"type":"string","enum":["development","production"]}, "sha256": {"type":"string","pattern":"^[0-9a-f]{64}$"}, "size": I, "release_id": S, "install_script_sha256": {"type":["string","null"]}}, ["app_id","target","version","channel","sha256","size","release_id","install_script_sha256"],
     desc="The signed fields. The message is UTF-8 text, one line each: `silicon-apps-release-v1`, then `app_id=`, `target=`, `version=`, `channel=`, `sha256=`, `size=`, `release_id=` and `install_script_sha256=` (the digest or `none`), each line ending in a newline."),
 "ResolutionSignature": obj({"key_id": S, "algorithm": {"type":"string","const":"ed25519"}, "signature": S, "keys_url": {"type":"string","const":"/.well-known/silicon-apps-keys.json"}, "manifest": ref("SignedManifest")}, ["key_id","algorithm","signature","keys_url","manifest"]),
 "SigningKey": obj({"key_id": S, "algorithm": {"type":"string","const":"ed25519"}, "public_key": {"type":"string","description":"Base64 Ed25519 public key."}, "status": {"type":"string","enum":["active","retired","revoked"]}, "first_seen_at": {"type":"string","format":"date-time"}, "endorsements": arr(obj({"key_id": S, "signature": S}, ["key_id","signature"]))}, ["key_id","algorithm","public_key","status","endorsements"]),
 "SigningKeys": obj({"issuer": {"type":"string","const":"silicon-apps"}, "service": S, "algorithm": {"type":"string","const":"ed25519"}, "active_key_id": S, "keys": arr(ref("SigningKey")), "revoked": arr(S), "messages": {"type":"object","description":"The exact message formats for release signatures, author signatures and endorsements."}, "docs": S}, ["issuer","algorithm","active_key_id","keys","revoked"],
     desc="Public keys that sign releases. An endorsement is a signature by an older key over `silicon-apps-key-endorsement-v1`, `key_id={key_id}` and `public_key={public_key}`, one per line: trust a new key when a key you already trust endorses it."),
 "AuthorKey": obj({"key_id": {"type":"string","description":"`ak_` and 16 hex characters of the SHA-256 of the public key."}, "name": S, "algorithm": {"type":"string","const":"ed25519"}, "public_key": S, "created_at": {"type":"string","format":"date-time"}, "status": {"type":"string","enum":["active","revoked"]}, "revoked_at": {"type":["string","null"],"format":"date-time"}, "revoked_reason": {"type":["string","null"]}}, ["key_id","name","algorithm","public_key","created_at","status"]),
 "CarouselItem": obj({"url": S, "kind": {"type":"string","enum":["image","video"]}, "alt": {"type":"string","maxLength":10000}}, ["url","kind"]),
 "Links": obj({"website": S, "developer_docs": S, "android": S, "ios": S, "custom": {"type":"array","maxItems":4,"items": obj({"label": S, "url": S, "logo": S}, ["label","url"])}}),
 "App": obj({
     "app_id": {"type":"string","description":"Permanent ID: 3 to 30 of a-z, 0-9, - and _."},
     "name": S, "description": S, "logo": S, "logo_alt": S, "banner": S, "banner_alt": S,
     "tags": arr(S), "visibility": {"type":"string","enum":["public","private"]},
     "domains": {"type":"array","items":S,"description":"Verified email domains with access. Authors only."},
     "account_ids": {"type":"array","items":S,"description":"Accounts shared with. Authors only."},
     "links": ref("Links"), "carousel": arr(ref("CarouselItem")),
     "published": B, "setup_step": {"type":"integer","minimum":1,"maximum":7},
     "created_at": {"type":"string","format":"date-time"}, "updated_at": {"type":"string","format":"date-time"},
     "authors": arr(ref("Author")), "targets": {"type":"array","items":{"type":"string","enum":TARGETS},"description":"Targets of the latest production release, or of the latest development release before the first production release."},
     "latest_production": {"anyOf":[ref("Release"),{"type":"null"}],"description":"The newest production release that is not withdrawn."}, "latest_development": {"anyOf":[ref("Release"),{"type":"null"}],"description":"The newest development release that is not withdrawn."},
     "signed": {"type":"boolean","description":"Every package of the current release is signed by Apps."}, "signed_by_author": {"type":"boolean","description":"Every package of the current release is also signed by an author's own key."}, "signed_by": {"type":"array","items":S,"description":"The c:id or si:id of each author who signed a package of the current release."}, "withdrawn_releases": {"type":"array","items":ref("WithdrawnRelease"),"description":"Releases the authors withdrew, with their reasons."},
     "rating": {"type":["number","null"]}, "review_count": I, "installs": I,
     "is_author": {"type":"boolean","description":"Whether the caller is an author."}, "is_admin": {"type":"boolean","description":"Whether the caller is the admin. The admin is never marked to others."}},
     ["app_id","name","description","tags","visibility","published","authors","targets","installs","is_author","is_admin"]),
 "AppPage": obj({"items": arr(ref("App")), "total": I, "limit": I, "offset": I, "next_offset": {"type":["integer","null"],"description":"Offset of the next page, or null on the last page."}, "sort": {"type":"string","enum":["relevance","rating","installs","name","updated","newest"]}}, ["items","total","limit","offset","next_offset","sort"]),
 "AuthorProfile": obj({"uuid": S, "id": S, "display_name": S, "items": arr(ref("App")), "total": I}, ["uuid","id","display_name","items","total"]),
 "Invite": obj({"id": S, "app_id": S, "to": {"type":"string","description":"c:id, si:id or email address."}, "account_uuid": {"type":["string","null"]}, "status": {"type":"string","enum":["pending","accepted","declined","cancelled"]}, "created_at": {"type":"string","format":"date-time"}}, ["id","app_id","to","status","created_at"]),
 "HistoryEntry": obj({"id": S, "at": {"type":"string","format":"date-time"}, "actor_uuid": S, "kind": S, "data": {}, "idempotency_key": {"type":["string","null"]}}, ["id","at","actor_uuid","kind","data"]),
 "Review": obj({"uuid": S, "id": S, "rating": {"type":"integer","minimum":1,"maximum":5}, "text": {"type":"string","maxLength":600}, "updated_at": {"type":"string","format":"date-time"}}, ["uuid","id","rating","text","updated_at"]),
 "Readiness": obj({"ready": B, "errors": arr(obj({"field": S, "message": S}, ["field","message"])), "required_commands": arr(S)}, ["ready","errors","required_commands"]),
 "Resolution": obj({"app_id": S, "release": ref("Release"), "package": ref("Package"), "download_path": S, "signature": ref("ResolutionSignature"), "author_signature": {"anyOf":[ref("AuthorSignature"),{"type":"null"}]}, "install_script": {"anyOf":[ref("InstallScript"),{"type":"null"}]}, "withdrawn": {"type":"array","items":ref("WithdrawnRelease"),"description":"Withdrawn releases on this channel. If yours is listed, move to this resolution."}}, ["app_id","release","package","download_path","signature","author_signature","install_script","withdrawn"]),
 "Media": obj({"url": S, "id": S, "kind": {"type":"string","enum":["image","video"]}, "size": I, "content_type": S}, ["url","id","kind","size","content_type"]),
 "AccountsWebhook": obj({"url": {"type":["string","null"]}, "secret_set": B, "events": {"type":["array","null"],"items":S}, "secret": {"type":"string","description":"Only when a secret was generated by this request."}}, extra=True),
 "TargetPopulation": obj({"items": arr(obj({"target": {"type":"string","enum":TARGETS}, "population": I, "runner_available": B}, ["target","population","runner_available"])), "total_population": I, "total_reach": I, "source": {"type":"string","const":"registered_accounts"}}, ["items","total_population","total_reach","source"]),
 "TokenResponse": obj({"access_token": S, "token_type": S, "expires_in": I, "refresh_token": S, "scope": S}, ["access_token","token_type","expires_in"], extra=True, desc="The official Silicon Accounts token response."),
 "Event": obj({"seq": {"type":"integer","description":"Position in the event log. Use it as Last-Event-ID or `after`."}, "id": {"type":"string","description":"Unique event ID."}, "type": {"type":"string","enum":EVENT_TYPES}, "app_id": {"type":["string","null"]}, "actor_uuid": S, "occurred_at": {"type":"string","format":"date-time"}, "data": {"description":"Event data. Release events carry the Release; validation steps carry the step, command, exit code, output and expectation."}}, ["seq","id","type","app_id","actor_uuid","occurred_at","data"]),
 "EventPage": obj({"items": arr(ref("Event")), "next_after": {"type":"integer","description":"Pass as `after` for the next page."}, "has_more": B, "cursor": {"type":"integer","description":"The newest event seq in the log."}}, ["items","next_after","has_more","cursor"]),
 "SubscriptionDelivery": {"oneOf": [obj({"mode": {"type":"string","const":"webhook"}, "url": {"type":"string","format":"uri","maxLength":2048}}, ["mode","url"]), obj({"mode": {"type":"string","const":"stream"}}, ["mode"])], "description": "Where events go: a signed webhook, or a stream you read at `stream_url`."},
 "Subscription": obj({
     "id": {"type":"string","description":"`sub_` followed by 32 hex characters."},
     "app_id": {"type":["string","null"],"description":"The app followed, or null for your account feed."},
     "types": {"type":"array","items":S,"description":"Exact event types, `group.*` or `*`."},
     "channels": {"type":"array","items":{"type":"string","enum":["production","development"]},"description":"Release channels to receive. Empty means both. Other events are not filtered."},
     "delivery": ref("SubscriptionDelivery"),
     "stream_url": S,
     "status": {"type":"string","enum":["active","paused","cancelled"]},
     "description": S, "cursor": {"type":"integer","description":"Last event seq sent on this subscription's stream."},
     "created_at": {"type":"string","format":"date-time"}, "updated_at": {"type":"string","format":"date-time"}, "cancelled_at": {"type":["string","null"],"format":"date-time"},
     "deliveries": obj({"pending": I, "delivered": I, "failed": I}, desc="Delivery counts. Only on GET of one subscription.")},
     ["id","app_id","types","channels","delivery","stream_url","status","description","cursor","created_at","updated_at"]),
 "SubscriptionResult": obj({"subscription": ref("Subscription"), "secret": {"type":"string","description":"The `whsec_` signing secret. Returned once, when it is generated. Save it now."}}, ["subscription"]),
 "SubscriptionCreate": obj({
     "app_id": {"type":["string","null"],"description":"An app you can see. Omit or null to follow your account feed: invitations to you, apps you author and public events of apps you installed."},
     "types": {"type":"array","items":S,"minItems":1,"maxItems":50,"description":"Defaults to `*` for authors and the account feed, and to app.published, release.created and release.promoted for everyone else, who can only receive those."},
     "channels": {"type":"array","items":{"type":"string","enum":["production","development"]}},
     "delivery": ref("SubscriptionDelivery"),
     "description": {"type":"string","maxLength":200}}, ["delivery"], extra=False),
 "SubscriptionUpdate": obj({
     "types": {"type":"array","items":S,"minItems":1,"maxItems":50},
     "channels": {"type":["array","null"],"items":{"type":"string","enum":["production","development"]}},
     "delivery": ref("SubscriptionDelivery"),
     "status": {"type":"string","enum":["active","paused"]},
     "description": {"type":"string","maxLength":200}}, extra=False),
 "Delivery": obj({"id": S, "event_id": S, "event_type": S, "event_seq": I, "status": {"type":"string","enum":["pending","delivered","failed"]}, "attempts": I, "next_attempt_at": {"type":["string","null"],"format":"date-time"}, "created_at": {"type":"string","format":"date-time"}, "delivered_at": {"type":["string","null"],"format":"date-time"}, "last_attempt_at": {"type":["string","null"],"format":"date-time"}, "last_status": {"type":["integer","null"]}, "last_error": {"type":["string","null"]}}, ["id","event_id","event_type","event_seq","status","attempts","created_at"]),
 "WebhookPayload": obj({"actor_uuid": S, "app_id": {"type":["string","null"]}, "data": {}, "event_id": S, "occurred_at": {"type":"string","format":"date-time"}, "seq": I, "subscription_id": S, "type": S}, ["actor_uuid","app_id","data","event_id","occurred_at","seq","subscription_id","type"], desc="Body of a subscription webhook delivery."),
 "Capabilities": obj({
     "service": S, "version": S,
     "api": obj({"current": S, "versions": arr(S), "header": S, "base_path": S, "negotiation": S}),
     "auth": obj({"methods": arr(S), "issuer": S, "audience": S}, extra=True),
     "targets": arr(obj({"target": S, "validation": {"type":"string","enum":["live","unreachable","not_configured"]}, "live": B}, ["target","validation","live"])),
     "validation_runner": obj({"configured": B, "reachable": {"type":["boolean","null"]}, "checked_at": {"type":["string","null"]}, "commands": arr(S)}),
     "search": {"type":"object"}, "streaming": {"type":"object"}, "subscriptions": {"type":"object"},
     "idempotency": {"type":"object"}, "rate_limits": {"type":"object"}, "errors": {"type":"object"},
     "signing": obj({"algorithm": S, "keys": S, "active_key_id": S}, extra=True), "withdrawal": {"type":"object"},
     "links": obj({"openapi": S, "agent_card": S, "mcp": S, "llms_txt": S, "docs": S}),
     "requirements": obj({"satisfied": {"type":"boolean","const":True}, "results": arr(obj({"requirement": S, "satisfied": B, "reason": S}, ["requirement","satisfied","reason"]))}, ["satisfied","results"], desc="Present when `require` was given and everything was met. When something is missing the answer is 422 `capabilities_missing` instead.")},
     ["service","version","api","auth","targets","streaming","subscriptions","idempotency","rate_limits","links"], extra=True),
 "AgentCard": obj({"protocolVersion": S, "name": S, "description": S, "url": S, "version": S, "provider": obj({"organization": S, "url": S}), "capabilities": obj({"streaming": B, "pushNotifications": B, "stateTransitionHistory": B}), "skills": arr(obj({"id": S, "name": S, "description": S, "tags": arr(S), "examples": arr(S)}, ["id","name","description","tags"]))}, ["protocolVersion","name","description","url","version","capabilities","skills"], extra=True, desc="A2A agent card."),
}

parameters = {
 "AppId": {"name":"app_id","in":"path","required":True,"description":"The app's permanent ID.","schema":{"type":"string","minLength":1,"maxLength":64}},
 "IdempotencyKey": {"name":"Idempotency-Key","in":"header","required":True,"description":"8 to 200 printable characters without spaces. Reuse it, with the same body, to retry a request whose result you did not receive.","schema":{"type":"string","minLength":8,"maxLength":200}},
 "SubscriptionId": {"name":"subscription_id","in":"path","required":True,"description":"The subscription's ID.","schema":{"type":"string"}},
 "Types": {"name":"types","in":"query","required":False,"description":"Comma-separated event types to include: exact types, `group.*` (for example `release.*`) or `*`.","schema":{"type":"string"},"example":"release.promoted,package.*"},
 "LastEventIdHeader": {"name":"Last-Event-ID","in":"header","required":False,"description":"Resume after this event seq. Browsers send it on reconnect.","schema":{"type":"string","pattern":"^[0-9]+$"}},
 "LastEventIdQuery": {"name":"last_event_id","in":"query","required":False,"description":"Resume after this event seq, for clients that cannot set headers.","schema":{"type":"integer","minimum":0}},
 "After": {"name":"after","in":"query","required":False,"description":"Return events after this seq. Defaults to 0, the oldest.","schema":{"type":"integer","minimum":0}},
 "EventLimit": {"name":"limit","in":"query","required":False,"description":"Events per page, 1 to 500.","schema":{"type":"integer","minimum":1,"maximum":500,"default":100}},
}

rate_headers = {
 "RateLimit-Limit": {"$ref":"#/components/headers/RateLimitLimit"},
 "RateLimit-Remaining": {"$ref":"#/components/headers/RateLimitRemaining"},
 "RateLimit-Reset": {"$ref":"#/components/headers/RateLimitReset"},
 "Apps-Version": {"$ref":"#/components/headers/AppsVersion"},
}
headers = {
 "RateLimitLimit": {"description":"Requests allowed per 60 second window for this client and kind (read or write).","schema":{"type":"integer"}},
 "RateLimitRemaining": {"description":"Requests left in the current window.","schema":{"type":"integer"}},
 "RateLimitReset": {"description":"Seconds until the window is full again.","schema":{"type":"integer"}},
 "RetryAfter": {"description":"Seconds to wait before retrying.","schema":{"type":"integer"}},
 "AppsVersion": {"description":"The API version used for this response.","schema":{"type":"string"}},
 "IdempotentReplayed": {"description":"`true` when this response replays an earlier request with the same Idempotency-Key.","schema":{"type":"string","enum":["true","false"]}},
}
def err(desc): return {"description": desc, "content": {"application/json": {"schema": ref("Error")}}}
responses = {
 "BadRequest": err("Invalid input, an unknown API version or an unknown event type."),
 "Unauthorized": err("Sign in: the request needs a valid Silicon Accounts token or session."),
 "Forbidden": err("Signed in, but not allowed: for example not an author or not the admin."),
 "NotFound": err("No such route or resource, or one you cannot see."),
 "Conflict": err("The current state does not allow this, or the Idempotency-Key belongs to another request."),
 "Unprocessable": err("The package failed validation. `error.details` holds the exact command results."),
 "Unavailable": err("A required service, such as the isolated runner or Silicon Accounts, is not available. Retry with the same Idempotency-Key."),
 "Gone": err("The release was withdrawn. `error.details` holds the reason and the replacement, if any."),
 "RateLimited": {"description":"Too many requests. Wait for Retry-After seconds.","headers":{"Retry-After":{"$ref":"#/components/headers/RetryAfter"},"RateLimit-Limit":{"$ref":"#/components/headers/RateLimitLimit"},"RateLimit-Remaining":{"$ref":"#/components/headers/RateLimitRemaining"},"RateLimit-Reset":{"$ref":"#/components/headers/RateLimitReset"}},"content":{"application/json":{"schema":ref("Error")}}},
}

PUBLIC = [{}]
AUTH = [{"bearerAuth": []}, {"sessionCookie": []}]
OPTIONAL = [{"bearerAuth": []}, {"sessionCookie": []}, {}]

paths = {}
def op(method, path, op_id, summary, tag, description, responses_, security=OPTIONAL, parameters_=None, body=None, errors=("BadRequest","NotFound","RateLimited"), mutation=False):
    o = {"operationId": op_id, "summary": summary, "description": description, "tags": [tag], "security": security}
    ps = list(parameters_ or [])
    if mutation: ps.append(param("IdempotencyKey"))
    if ps: o["parameters"] = ps
    if body: o["requestBody"] = body
    rs = dict(responses_)
    for e in errors:
        code = {"BadRequest":"400","Unauthorized":"401","Forbidden":"403","NotFound":"404","Conflict":"409","Gone":"410","Unprocessable":"422","RateLimited":"429","Unavailable":"503"}[e]
        rs.setdefault(code, resp(e))
    for code, r in rs.items():
        if code.startswith("2") and "$ref" not in r:
            r.setdefault("headers", {}).update(rate_headers)
            if mutation: r["headers"]["Idempotent-Replayed"] = {"$ref":"#/components/headers/IdempotentReplayed"}
    o["responses"] = rs
    paths.setdefault(path, {})[method.lower()] = o

AUTHED_ERR = ("BadRequest","Unauthorized","Forbidden","NotFound","RateLimited")
MUT_ERR = ("BadRequest","Unauthorized","Forbidden","NotFound","Conflict","RateLimited","Unavailable")

# Discovery
op("GET","/health","getHealth","Check the service is up","Discovery","Liveness and version. Not rate limited.",{"200":ok(ref("Health"))},security=PUBLIC,errors=("BadRequest",))
op("GET","/openapi.json","getOpenApi","Get this OpenAPI document","Discovery","The OpenAPI 3.1 description of every public route.",{"200":ok({"type":"object"},"The OpenAPI document.")},security=PUBLIC,errors=("RateLimited",))
op("GET","/v1/openapi.json","getOpenApiV1","Get this OpenAPI document under /v1","Discovery","The same document as /openapi.json.",{"200":ok({"type":"object"},"The OpenAPI document.")},security=PUBLIC,errors=("RateLimited",))
op("GET","/.well-known/agent.json","getAgentCard","Get the A2A agent card","Discovery","The A2A agent card: skills, capabilities, auth and links to this document, llms.txt, the docs and the MCP endpoint.",{"200":ok(ref("AgentCard"))},security=PUBLIC,errors=("RateLimited",))
op("GET","/.well-known/agent-card.json","getAgentCardA2a","Get the A2A agent card (A2A 0.3 path)","Discovery","The same card as /.well-known/agent.json, at the path newer A2A clients read.",{"200":ok(ref("AgentCard"))},security=PUBLIC,errors=("RateLimited",))
op("GET","/v1/capabilities","getCapabilities","Get capabilities and negotiate requirements","Discovery",
   "API versions, auth methods, targets and which validation workers are live, search parameters, streaming, subscriptions, idempotency, rate limits and links. Pass `require` to ask whether this server meets your needs: when it does, the answer is in `requirements`; when it does not, you get 422 `capabilities_missing` with what is missing in `error.details.missing`.\n\nSend `Apps-Version` on any request to choose an API version. It may list several in order of preference. The response's `Apps-Version` header names the version used. An unknown version returns 400 `unsupported_api_version`.",
   {"200":ok(ref("Capabilities"))},security=PUBLIC,
   parameters_=[{"name":"require","in":"query","required":False,"description":"Comma-separated requirements: streaming, subscriptions, webhooks, idempotency, search, rate_limits, openapi, agent_card, signing, author_signatures, withdrawal, mcp, version:V, auth:METHOD, delivery:MODE, event:TYPE, target:TARGET.","schema":{"type":"string"},"example":"streaming,subscriptions,target:linux-x86_64"},
                {"name":"Apps-Version","in":"header","required":False,"description":"API versions you accept, in order of preference.","schema":{"type":"string"},"example":"2026-10-09"}],
   errors=("BadRequest","Unprocessable","RateLimited"))
op("GET","/.well-known/silicon-apps-keys.json","getSigningKeys","Get the release signing keys","Discovery",
   "The Ed25519 public keys that sign every release package Apps serves, with endorsements that let a client move from a key it trusts to a newer one, the revoked key IDs, and the exact signed message formats.",
   {"200":ok(ref("SigningKeys"))},security=PUBLIC,errors=("RateLimited",))

# Accounts and session
op("GET","/v1/session","getSession","Get the browser session","Accounts","Whether the request is signed in, and as whom.",{"200":ok(ref("Session"))},errors=("Unauthorized","RateLimited"))
op("GET","/v1/me","getMe","Get the signed-in account","Accounts","The account behind the token or session, with verified emails when the Email scope was granted.",{"200":ok(ref("Identity"))},security=AUTH,errors=("Unauthorized","RateLimited"))
op("GET","/v1/targets","getTargets","Get target populations","Catalog","Supported targets, how many signed-in accounts were seen on each, and whether an upload worker is configured. `targets` selects targets for `total_reach`.",{"200":ok(ref("TargetPopulation"))},parameters_=[{"name":"targets","in":"query","required":False,"description":"Comma-separated targets to count reach for.","schema":{"type":"string"}}],errors=("Unauthorized","RateLimited"))

# Catalog
op("GET","/v1/apps","searchApps","Search and list apps","Catalog",
   "Published apps you can see, best match first. Exact ID and name matches rank ahead of prefixes, substrings and typo matches; rating only breaks ties. `mine=true` lists apps you author, drafts included.",
   {"200":ok(ref("AppPage"))},
   parameters_=[
     {"name":"q","in":"query","required":False,"description":"Search text over IDs, names, tags and descriptions, with typo tolerance. Up to 512 bytes.","schema":{"type":"string","maxLength":512}},
     {"name":"tags","in":"query","required":False,"description":"Comma-separated tags. An app must have every listed tag (case-insensitive). Up to 20.","schema":{"type":"string"}},
     {"name":"target","in":"query","required":False,"description":"Only apps whose current release has a package for this target.","schema":{"type":"string","enum":TARGETS}},
     {"name":"visibility","in":"query","required":False,"description":"`private` needs sign-in and lists private apps shared with you.","schema":{"type":"string","enum":["public","private"]}},
     {"name":"mine","in":"query","required":False,"description":"`true` lists apps you author, including drafts. Needs sign-in.","schema":{"type":"boolean"}},
     {"name":"sort","in":"query","required":False,"description":"`relevance` (default), `rating`, `installs`, `name`, `updated` or `newest`.","schema":{"type":"string","enum":["relevance","rating","installs","name","updated","newest"],"default":"relevance"}},
     {"name":"limit","in":"query","required":False,"description":"Page size.","schema":{"type":"integer","minimum":1,"maximum":100,"default":50}},
     {"name":"offset","in":"query","required":False,"description":"Items to skip. Use `next_offset` from the previous page.","schema":{"type":"integer","minimum":0,"default":0}}],
   errors=("BadRequest","Unauthorized","RateLimited"))
op("POST","/v1/apps","createApp","Create an app","Authoring",
   "Creates an unpublished app with you as its first author and returns its `app_secret` once. Registers the app with Silicon Accounts.",
   {"200":ok(obj({"app":ref("App"),"app_secret":{"type":"string","description":"Shown once. Save it now."}},["app","app_secret"]))},security=AUTH,
   body=json_body(obj({"app_id":{"type":"string","pattern":"^[a-z0-9_-]{3,30}$"},"name":{"type":"string","minLength":1,"maxLength":120},"description":{"type":"string","maxLength":600},"logo":S},["app_id","name"])),
   errors=MUT_ERR,mutation=True)
op("GET","/v1/apps/availability/{app_id}","getAvailability","Check whether an app ID is free","Catalog","`available` is false for taken, reserved and invalid IDs.",{"200":ok(obj({"available":B},["available"]))},parameters_=[param("AppId")],errors=("RateLimited","Unavailable"))
op("GET","/v1/apps/{app_id}","getApp","Get an app","Catalog","An app's page. Drafts are visible only to authors; private apps only to accounts they are shared with.",{"200":ok(ref("App"))},parameters_=[param("AppId")],errors=("Unauthorized","NotFound","RateLimited"))
op("PATCH","/v1/apps/{app_id}","updateApp","Update app details","Authoring","Change details, links, media and the saved setup step. Authors only.",{"200":ok(ref("App"))},security=AUTH,parameters_=[param("AppId")],
   body=json_body(obj({"name":S,"description":{"type":"string","maxLength":600},"tags":{"type":"array","maxItems":20,"items":{"type":"string","maxLength":60}},"logo":S,"logo_alt":S,"banner":S,"banner_alt":S,"carousel":{"type":"array","maxItems":20,"items":ref("CarouselItem")},"links":ref("Links"),"setup_step":{"type":"integer","minimum":1,"maximum":7}},extra=False)),
   errors=MUT_ERR,mutation=True)
op("PUT","/v1/apps/{app_id}/access","setAccess","Set public or private access","Authoring","Replace visibility, shared accounts and allowed verified email domains. Admin only.",{"200":ok(ref("App"))},security=AUTH,parameters_=[param("AppId")],
   body=json_body(obj({"visibility":{"type":"string","enum":["public","private"]},"domains":{"type":"array","maxItems":100,"items":S},"account_ids":{"type":"array","items":S}},["visibility"])),errors=MUT_ERR,mutation=True)
op("GET","/v1/apps/{app_id}/readiness","getReadiness","Check what publishing still needs","Authoring","Missing requirements for publishing. Authors only.",{"200":ok(ref("Readiness"))},security=AUTH,parameters_=[param("AppId")],errors=AUTHED_ERR)
op("POST","/v1/apps/{app_id}/publish","publishApp","Publish an app","Authoring","Publishes immediately when ready. There is no review.",{"200":ok(ref("App"))},security=AUTH,parameters_=[param("AppId")],body=json_body(obj({}),required=False),errors=MUT_ERR,mutation=True)
op("POST","/v1/apps/{app_id}/secret/rotate","rotateAppSecret","Rotate the app secret","Authoring","Replaces the app secret and returns the new one once. The old one stops working.",{"200":ok(obj({"app_secret":S},["app_secret"]))},security=AUTH,parameters_=[param("AppId")],body=json_body(obj({}),required=False),errors=MUT_ERR,mutation=True)

# Authors
op("GET","/v1/apps/{app_id}/authors","listAuthors","List authors","Authors","Every accepted author.",{"200":ok(obj({"items":arr(ref("Author"))},["items"]))},parameters_=[param("AppId")],errors=("Unauthorized","NotFound","RateLimited"))
op("POST","/v1/apps/{app_id}/authors/leave","leaveApp","Leave an app","Authors","Remove yourself as an author. The last author cannot leave.",{"200":ok(obj({"status":{"type":"string","const":"left"}},["status"]))},security=AUTH,parameters_=[param("AppId")],body=json_body(obj({}),required=False),errors=MUT_ERR,mutation=True)
op("DELETE","/v1/apps/{app_id}/authors/{uuid}","removeAuthor","Remove an author","Authors","Admin only. You cannot remove yourself; leave instead.",{"200":ok(obj({"status":{"type":"string","const":"removed"}},["status"]))},security=AUTH,
   parameters_=[param("AppId"),{"name":"uuid","in":"path","required":True,"description":"The author's account UUID.","schema":{"type":"string"}}],errors=MUT_ERR,mutation=True)
op("POST","/v1/apps/{app_id}/admin","transferAdmin","Transfer adminship","Authors","Admin only. The new admin must already be an author.",{"200":ok(obj({"status":{"type":"string","const":"transferred"}},["status"]))},security=AUTH,parameters_=[param("AppId")],body=json_body(obj({"uuid":S},["uuid"])),errors=MUT_ERR,mutation=True)
op("GET","/v1/apps/{app_id}/invites","listAppInvites","List an app's invitations","Authors","Authors only.",{"200":ok(obj({"items":arr(ref("Invite"))},["items"]))},security=AUTH,parameters_=[param("AppId")],errors=AUTHED_ERR)
op("POST","/v1/apps/{app_id}/invites","inviteAuthor","Invite an author","Authors","Invite a Carbon or Silicon by c:id, si:id or email. They become an author when they accept.",{"200":ok(ref("Invite"))},security=AUTH,parameters_=[param("AppId")],body=json_body(obj({"to":S},["to"])),errors=MUT_ERR,mutation=True)
op("DELETE","/v1/apps/{app_id}/invites/{invite_id}","cancelInvite","Cancel an invitation","Authors","Any author can cancel a pending invitation.",{"200":ok(obj({"status":{"type":"string","const":"cancelled"}},["status"]))},security=AUTH,
   parameters_=[param("AppId"),{"name":"invite_id","in":"path","required":True,"description":"The invitation's ID.","schema":{"type":"string"}}],errors=MUT_ERR,mutation=True)
op("GET","/v1/invites","listMyInvites","List invitations to you","Authors","Pending invitations addressed to your UUID or a verified email.",{"200":ok(obj({"items":arr(ref("Invite"))},["items"]))},security=AUTH,errors=("Unauthorized","RateLimited"))
for action, const in (("accept","accepted"),("decline","declined")):
    op("POST",f"/v1/invites/{{invite_id}}/{action}",f"{action}Invite",f"{action.capitalize()} an invitation","Authors",f"{action.capitalize()} an invitation addressed to you.",{"200":ok(obj({"status":{"type":"string","const":const}},["status"]))},security=AUTH,
       parameters_=[{"name":"invite_id","in":"path","required":True,"description":"The invitation's ID.","schema":{"type":"string"}}],body=json_body(obj({}),required=False),errors=MUT_ERR,mutation=True)
op("GET","/v1/apps/{app_id}/history","getHistory","Read an app's history","Authors","Every change, newest first, with the idempotency key that made it. Authors only.",{"200":ok(obj({"items":arr(ref("HistoryEntry")),"total":I},["items","total"]))},security=AUTH,
   parameters_=[param("AppId"),{"name":"limit","in":"query","required":False,"description":"Entries per page, up to 500.","schema":{"type":"integer","minimum":1,"maximum":500,"default":100}},{"name":"offset","in":"query","required":False,"description":"Entries to skip.","schema":{"type":"integer","minimum":0,"default":0}}],errors=AUTHED_ERR)
op("GET","/v1/authors/{uuid}","getAuthorProfile","Get an author's profile","Catalog","An author's current ID and the published apps of theirs you can see.",{"200":ok(ref("AuthorProfile"))},
   parameters_=[{"name":"uuid","in":"path","required":True,"description":"The author's account UUID.","schema":{"type":"string"}},{"name":"limit","in":"query","required":False,"description":"Apps per page.","schema":{"type":"integer","minimum":1,"maximum":100,"default":24}},{"name":"offset","in":"query","required":False,"description":"Apps to skip.","schema":{"type":"integer","minimum":0,"default":0}}],errors=("NotFound","RateLimited"))

# Packages and releases
op("GET","/v1/apps/{app_id}/packages","listPackages","List packages","Releases","Uploaded packages with their command results. Authors only.",{"200":ok(obj({"items":arr(ref("Package"))},["items"]))},security=AUTH,parameters_=[param("AppId")],errors=AUTHED_ERR)
op("POST","/v1/apps/{app_id}/packages/{target}","uploadPackage","Upload a package for a target","Releases",
   "Send the raw `.tar.gz`. The service checks the archive and manifest, then runs `--help`, `accounts --json` and `login status --json` in an isolated runner for that target. Each step is published on the app's event stream as `package.validation_step`. A failure returns 422 with the exact command results.\n\nTo sign the package as its author, send `X-Apps-Author-Key-Id` (a key from `/v1/keys`) and `X-Apps-Author-Signature`: a base64 Ed25519 signature over the lines `silicon-apps-author-package-v1`, `app_id=`, `target=`, `sha256=`, `size=` and `install_script_sha256=` (the digest or `none`), each ending in a newline. A signature that does not verify returns 422 `invalid_author_signature` before any command runs.",
   {"200":ok(ref("Package"))},security=AUTH,
   parameters_=[param("AppId"),{"name":"target","in":"path","required":True,"description":"The package target.","schema":{"type":"string","enum":TARGETS}},
                {"name":"X-Apps-Author-Key-Id","in":"header","required":False,"description":"Your author key's ID, sent with X-Apps-Author-Signature.","schema":{"type":"string"}},
                {"name":"X-Apps-Author-Signature","in":"header","required":False,"description":"Base64 Ed25519 signature over the author package message.","schema":{"type":"string"}}],
   body={"required":True,"content":{"application/gzip":{"schema":{"type":"string","format":"binary"}}}},
   errors=("BadRequest","Unauthorized","Forbidden","NotFound","Conflict","Unprocessable","RateLimited","Unavailable"),mutation=True)
op("GET","/v1/apps/{app_id}/packages/{package_id}/download","downloadPackage","Download a package","Releases","The package bytes. Visibility is checked on every download; check the SHA-256 and the signature from `resolve` before you run anything. Packages that belong only to withdrawn releases return 410 to everyone but the app's authors.",
   {"200":{"description":"The package archive.","content":{"application/gzip":{"schema":{"type":"string","format":"binary"}}}}},
   parameters_=[param("AppId"),{"name":"package_id","in":"path","required":True,"description":"The package's ID.","schema":{"type":"string"}}],errors=("Unauthorized","NotFound","Gone","RateLimited","Unavailable"))
op("GET","/v1/apps/{app_id}/releases","listReleases","List releases","Releases","Release history, newest first.",{"200":ok(obj({"items":arr(ref("Release"))},["items"]))},
   parameters_=[param("AppId"),{"name":"channel","in":"query","required":False,"description":"Only this channel.","schema":{"type":"string","enum":["production","development"]}}],errors=("Unauthorized","NotFound","RateLimited"))
op("POST","/v1/apps/{app_id}/releases","createRelease","Create a development release","Releases","A new development release from validated packages, at most one per target.",{"200":ok(ref("Release"))},security=AUTH,parameters_=[param("AppId")],
   body=json_body(obj({"version":{"type":"string"},"package_ids":{"type":"array","minItems":1,"items":S},"notes":S},["version","package_ids"])),errors=MUT_ERR,mutation=True)
op("POST","/v1/apps/{app_id}/releases/{release_id}/promote","promoteRelease","Promote to production","Releases","Creates a production release with the development release's packages and its own production version.",{"200":ok(ref("Release"))},security=AUTH,
   parameters_=[param("AppId"),{"name":"release_id","in":"path","required":True,"description":"A development release's ID.","schema":{"type":"string"}}],body=json_body(obj({"version":S},["version"])),errors=MUT_ERR,mutation=True)
op("POST","/v1/apps/{app_id}/releases/{release_id}/withdraw","withdrawRelease","Withdraw a bad release","Releases",
   "Stops serving a release, with a reason. Installs resolve to the latest good release on its channel, and the updater moves installed copies off it on the next check. Records `release.withdrawn` in the history and on event streams. A withdrawn release cannot be promoted, and withdrawing is final: publish a new release to replace it. Authors only.",
   {"200":ok({"allOf":[ref("Release"),obj({"replacement":{"anyOf":[obj({"release_id":S,"version":S},["release_id","version"]),{"type":"null"}],"description":"The latest good release on the channel, which installs now get, or null."}},["replacement"])]})},security=AUTH,
   parameters_=[param("AppId"),{"name":"release_id","in":"path","required":True,"description":"The release's ID.","schema":{"type":"string"}}],
   body=json_body(obj({"reason":{"type":"string","minLength":1,"maxLength":500,"description":"Why it is withdrawn, shown on the app page and to installers."}},["reason"],extra=False)),errors=MUT_ERR,mutation=True)
op("GET","/v1/apps/{app_id}/resolve","resolveRelease","Resolve a release for a target","Releases","The release and package to install, with the API's signature over its manifest. Defaults to the latest production release that is not withdrawn. An exact version that was withdrawn returns 410 `release_withdrawn` with the reason and the replacement.",{"200":ok(ref("Resolution"))},
   parameters_=[param("AppId"),{"name":"target","in":"query","required":True,"description":"Your target.","schema":{"type":"string","enum":TARGETS}},{"name":"channel","in":"query","required":False,"description":"Release channel.","schema":{"type":"string","enum":["production","development"],"default":"production"}},{"name":"version","in":"query","required":False,"description":"An exact version on that channel.","schema":{"type":"string"}}],
   errors=("BadRequest","Unauthorized","NotFound","Gone","RateLimited","Unavailable"))
op("POST","/v1/apps/{app_id}/installs","recordInstall","Record an install","Releases","Count a completed install. Signed-in installs also record the account's target.",{"200":ok(obj({"installs":I},["installs"]))},parameters_=[param("AppId")],
   body=json_body(obj({"release_id":S,"package_id":S},["release_id","package_id"])),errors=("BadRequest","Unauthorized","NotFound","Conflict","RateLimited"),mutation=True)

# Reviews and webhooks
op("GET","/v1/apps/{app_id}/reviews","listReviews","List reviews","Reviews","Reviews and the average rating.",{"200":ok(obj({"items":arr(ref("Review")),"rating":{"type":["number","null"]},"count":I},["items","rating","count"]))},parameters_=[param("AppId")],errors=("Unauthorized","NotFound","RateLimited"))
op("PUT","/v1/apps/{app_id}/review","putReview","Save your review","Reviews","One review per account: 1 to 5 stars and up to 600 characters.",{"200":ok(ref("Review"))},security=AUTH,parameters_=[param("AppId")],
   body=json_body(obj({"rating":{"type":"integer","minimum":1,"maximum":5},"text":{"type":"string","maxLength":600}},["rating"])),errors=MUT_ERR,mutation=True)
op("DELETE","/v1/apps/{app_id}/review","deleteReview","Remove your review","Reviews","Removes your own review, even after losing access to a private app.",{"200":ok(obj({"status":{"type":"string","const":"removed"}},["status"]))},security=AUTH,parameters_=[param("AppId")],errors=MUT_ERR,mutation=True)
op("GET","/v1/apps/{app_id}/webhook","getAccountsWebhook","Get the Accounts webhook","Authoring","The app's Silicon Accounts webhook settings. Authors only.",{"200":ok(ref("AccountsWebhook"))},security=AUTH,parameters_=[param("AppId")],errors=AUTHED_ERR+("Unavailable",))
op("PUT","/v1/apps/{app_id}/webhook","setAccountsWebhook","Set the Accounts webhook","Authoring","Save the endpoint and account events Silicon Accounts sends to the app. Stored and delivered by Accounts.",{"200":ok(ref("AccountsWebhook"))},security=AUTH,parameters_=[param("AppId")],
   body=json_body(obj({"url":S,"events":arr(S)},["url"])),errors=MUT_ERR,mutation=True)
op("POST","/v1/apps/{app_id}/webhook/rotate","rotateAccountsWebhookSecret","Rotate the Accounts webhook secret","Authoring","Generates a new `whsec_` secret, shown once.",{"200":ok(obj({"webhook_secret":S},["webhook_secret"]))},security=AUTH,parameters_=[param("AppId")],body=json_body(obj({}),required=False),errors=MUT_ERR,mutation=True)
op("POST","/v1/apps/{app_id}/media","uploadMedia","Upload media","Authoring","Raw PNG, JPEG, WebP, GIF, MP4 or WebM up to 100 MiB with its Content-Type. Save the returned URL in a media field.",{"200":ok(ref("Media"))},security=AUTH,parameters_=[param("AppId")],
   body={"required":True,"content":{m:{"schema":{"type":"string","format":"binary"}} for m in ["image/png","image/jpeg","image/webp","image/gif","video/mp4","video/webm"]}},errors=MUT_ERR,mutation=True)
op("GET","/v1/apps/{app_id}/media/{media_id}","getMedia","Get media","Catalog","Media bytes. Visibility is checked on every read.",{"200":{"description":"The media file.","content":{m:{"schema":{"type":"string","format":"binary"}} for m in ["image/png","image/jpeg","image/webp","image/gif","video/mp4","video/webm"]}}},
   parameters_=[param("AppId"),{"name":"media_id","in":"path","required":True,"description":"The media's SHA-256.","schema":{"type":"string","pattern":"^[0-9a-f]{64}$"}}],errors=("NotFound","RateLimited"))

# Events
SSE_DESC = ("Server-sent events. Each event has `id` (its seq), `event` (its type) and `data` (the Event as JSON). "
            "The stream starts with a `: ready cursor=N` comment and sends a `: heartbeat` comment every 15 seconds. "
            "Without Last-Event-ID it starts at the newest event. It closes after 30 minutes; reconnect with Last-Event-ID to continue.")
sse_ok = {"200":{"description":"An event stream.","content":{"text/event-stream":{"schema":{"type":"string"},"example":"id: 42\nevent: release.promoted\ndata: {\"seq\":42,\"type\":\"release.promoted\",\"app_id\":\"briefcase\",\"data\":{\"channel\":\"production\",\"version\":\"1.4.0\"}}\n\n"}}}}
op("GET","/v1/apps/{app_id}/events","listAppEvents","List an app's events","Events","The app's event log as JSON pages: releases, package validation steps with the three commands' results, author invitations, joins and leaves, access changes and more. Authors only.",
   {"200":ok(ref("EventPage"))},security=AUTH,parameters_=[param("AppId"),param("After"),param("Types"),param("EventLimit")],errors=AUTHED_ERR)
op("GET","/v1/apps/{app_id}/events/stream","streamAppEvents","Stream an app's events","Events","Live events for an app's authors. "+SSE_DESC,
   sse_ok,security=AUTH,parameters_=[param("AppId"),param("Types"),param("LastEventIdHeader"),param("LastEventIdQuery")],errors=AUTHED_ERR)
op("GET","/v1/events","listMyEvents","List your account's events","Events","Your account feed as JSON pages: invitations to you, everything about apps you author, and public events (releases, publishing) of apps you installed. With `subscription`, that subscription's events.",
   {"200":ok(ref("EventPage"))},security=AUTH,parameters_=[param("After"),param("Types"),param("EventLimit"),{"name":"subscription","in":"query","required":False,"description":"Read through one of your subscriptions' filters.","schema":{"type":"string"}}],errors=("BadRequest","Unauthorized","NotFound","Conflict","RateLimited"))
op("GET","/v1/events/stream","streamMyEvents","Stream your account's events","Events","Live events for the signed-in account. With `subscription`, the subscription's events; without Last-Event-ID it resumes where that subscription's stream last stopped. "+SSE_DESC,
   sse_ok,security=AUTH,parameters_=[param("Types"),param("LastEventIdHeader"),param("LastEventIdQuery"),{"name":"subscription","in":"query","required":False,"description":"Stream one of your subscriptions.","schema":{"type":"string"}}],errors=("BadRequest","Unauthorized","NotFound","Conflict","RateLimited"))

# Subscriptions
op("GET","/v1/subscriptions","listSubscriptions","List your subscriptions","Subscriptions","Active and paused subscriptions by default.",{"200":ok(obj({"items":arr(ref("Subscription"))},["items"]))},security=AUTH,
   parameters_=[{"name":"status","in":"query","required":False,"description":"Filter by status.","schema":{"type":"string","enum":["active","paused","cancelled","all"]}}],errors=("BadRequest","Unauthorized","RateLimited"))
op("POST","/v1/subscriptions","createSubscription","Subscribe to events","Subscriptions",
   "Follow an app you can see, or your account feed. A webhook subscription returns its `whsec_` signing secret once. Each delivery is a POST with `X-Apps-Event-Id`, `X-Apps-Event-Type`, `X-Apps-Delivery-Id`, `X-Apps-Subscription-Id`, `X-Apps-Timestamp` and `X-Apps-Signature: v1=<hex HMAC-SHA256(secret, \"{timestamp}.{raw body}\")>`. A 2xx answer within 10 seconds is a success; failures retry after 10 s, 30 s, 1 min, 5 min, 15 min, 30 min, then hourly, for 72 hours. Redirects are not followed. Production deliveries go only to public HTTPS endpoints.",
   {"201":ok(ref("SubscriptionResult"),"Created.")},security=AUTH,
   body={"required":True,"content":{"application/json":{"schema":ref("SubscriptionCreate"),"example":{"app_id":"briefcase","types":["release.promoted"],"delivery":{"mode":"webhook","url":"https://example.com/hooks/apps"},"description":"Tell me when briefcase ships to production"}}}},
   errors=("BadRequest","Unauthorized","Forbidden","NotFound","Conflict","RateLimited"),mutation=True)
op("GET","/v1/subscriptions/{subscription_id}","getSubscription","Get a subscription","Subscriptions","One of your subscriptions, with delivery counts.",{"200":ok(ref("Subscription"))},security=AUTH,parameters_=[param("SubscriptionId")],errors=("Unauthorized","NotFound","RateLimited"))
op("PATCH","/v1/subscriptions/{subscription_id}","updateSubscription","Update, pause or resume a subscription","Subscriptions","Change types, channels, delivery or description, or set `status` to `paused` or `active`. Deliveries due while paused are held and go out on resume within 72 hours of their event. Switching to webhook delivery returns a new secret once.",
   {"200":ok(ref("SubscriptionResult"))},security=AUTH,parameters_=[param("SubscriptionId")],body=json_body(ref("SubscriptionUpdate")),errors=("BadRequest","Unauthorized","Forbidden","NotFound","Conflict","RateLimited"),mutation=True)
op("DELETE","/v1/subscriptions/{subscription_id}","cancelSubscription","Cancel a subscription","Subscriptions","Stops the subscription for good. Pending deliveries fail. Cancelling again returns the same result.",
   {"200":ok(obj({"subscription":ref("Subscription")},["subscription"]))},security=AUTH,parameters_=[param("SubscriptionId")],errors=("Unauthorized","NotFound","RateLimited"),mutation=True)
op("GET","/v1/subscriptions/{subscription_id}/deliveries","listDeliveries","List webhook deliveries","Subscriptions","Recent deliveries, newest first, with attempts and the exact last error.",{"200":ok(obj({"items":arr(ref("Delivery"))},["items"]))},security=AUTH,
   parameters_=[param("SubscriptionId"),{"name":"status","in":"query","required":False,"description":"Filter by status.","schema":{"type":"string","enum":["pending","delivered","failed"]}},{"name":"limit","in":"query","required":False,"description":"Deliveries to return.","schema":{"type":"integer","minimum":1,"maximum":100,"default":50}}],errors=("BadRequest","Unauthorized","NotFound","RateLimited"))
op("POST","/v1/subscriptions/{subscription_id}/secret/rotate","rotateSubscriptionSecret","Rotate a subscription's signing secret","Subscriptions","A new `whsec_` secret, shown once. It signs every attempt from now on, including retries.",{"200":ok(obj({"secret":S},["secret"]))},security=AUTH,parameters_=[param("SubscriptionId")],errors=("Unauthorized","NotFound","Conflict","RateLimited"),mutation=True)
op("POST","/v1/subscriptions/{subscription_id}/ping","pingSubscription","Send a test delivery","Subscriptions","Queues a signed `ping` delivery to this webhook subscription only.",{"200":ok(obj({"delivery_id":S,"event_id":S,"status":{"type":"string","const":"pending"}},["delivery_id","event_id","status"]))},security=AUTH,parameters_=[param("SubscriptionId")],errors=("Unauthorized","NotFound","Conflict","RateLimited"),mutation=True)

# Author keys
op("GET","/v1/keys","listAuthorKeys","List your author keys","Releases","Your registered author signing keys, active and revoked.",{"200":ok(obj({"items":arr(ref("AuthorKey"))},["items"]))},security=AUTH,errors=("Unauthorized","RateLimited"))
op("POST","/v1/keys","addAuthorKey","Register an author key","Releases","Register an Ed25519 public key you sign packages with. Its ID is derived from the key. Up to 20 active keys per account. Keep the private key to yourself; Apps never sees it.",
   {"201":ok(obj({"key":ref("AuthorKey")},["key"]),"Created.")},security=AUTH,body=json_body(obj({"public_key":{"type":"string","description":"Base64 Ed25519 public key (32 bytes)."},"name":{"type":"string","maxLength":100}},["public_key"],extra=False)),
   errors=("BadRequest","Unauthorized","Conflict","RateLimited"),mutation=True)
op("DELETE","/v1/keys/{key_id}","revokeAuthorKey","Revoke an author key","Releases","Revoke a key, for example when it leaks. New uploads cannot be signed with it. Revoking again returns the same key.",
   {"200":ok(obj({"key":ref("AuthorKey")},["key"]))},security=AUTH,parameters_=[{"name":"key_id","in":"path","required":True,"description":"The author key's ID.","schema":{"type":"string"}}],
   body=json_body(obj({"reason":{"type":"string","maxLength":500}},extra=False),required=False),errors=("BadRequest","Unauthorized","NotFound","RateLimited"),mutation=True)

# Other
op("POST","/v1/platforms","registerPlatform","Record your platform","Accounts","Records the signed-in account's target for target populations.",{"200":ok(obj({"registered":B,"target":S},["registered","target"]))},security=AUTH,body=json_body(obj({"target":{"type":"string","enum":TARGETS}},["target"])),errors=MUT_ERR,mutation=True)
op("POST","/v1/reports","reportBug","Report a bug","Accounts","Mails a bug report to the maintainers, with an optional pull request link.",{"200":ok(obj({"id":S,"status":{"type":"string","const":"queued"}},["id","status"]))},body=json_body(obj({"message":{"type":"string","minLength":1,"maxLength":20000},"pr":S},["message"])),errors=("BadRequest","Unauthorized","Conflict","RateLimited","Unavailable"),mutation=True)
op("POST","/v1/telemetry","recordTelemetry","Record a telemetry event","Accounts","A sanitized usage event. `X-Apps-Telemetry: off` opts out.",{"200":ok(obj({"accepted":B,"reason":S},["accepted"]))},
   body=json_body(obj({"step":S,"progress":{},"event":S,"path":S,"target":S,"status_code":I,"duration_ms":I,"item_count":I,"byte_count":I,"error_code":S},["step","progress"])),errors=("BadRequest","Conflict","RateLimited"),mutation=True)

# Auth
redirect = {"303":{"description":"Redirect.","headers":{"Location":{"description":"Where to go next.","schema":{"type":"string"}}}}}
op("GET","/v1/auth/login","startLogin","Start browser sign-in","Auth","Redirects to Silicon Accounts with PKCE and a browser-bound state.",redirect,security=PUBLIC,
   parameters_=[{"name":"return_to","in":"query","required":False,"description":"A local path to return to.","schema":{"type":"string","default":"/"}}],errors=("BadRequest","RateLimited","Unavailable"))
op("GET","/v1/auth/callback","finishLogin","Finish browser sign-in","Auth","Silicon Accounts redirects here. Sets the HttpOnly session cookie and redirects to `return_to`.",redirect,security=PUBLIC,
   parameters_=[{"name":"state","in":"query","required":True,"description":"The sign-in state.","schema":{"type":"string"}},{"name":"code","in":"query","required":False,"description":"The authorization code.","schema":{"type":"string"}}],errors=("BadRequest","Forbidden","RateLimited","Unavailable"))
op("POST","/v1/auth/exchange","exchangeToken","Exchange a single-use token","Auth","Exchange a single-use token from `silicon-accounts login --app silicon-apps` for Apps tokens. No browser needed. Do not retry a consumed token; Idempotency-Key is not used here.",
   {"200":ok(ref("TokenResponse"))},security=PUBLIC,body=json_body(obj({"slt":S},["slt"])),errors=("BadRequest","Unauthorized","RateLimited","Unavailable"))
op("POST","/v1/auth/refresh","refreshToken","Refresh tokens","Auth","Exchange a rotating refresh token. Do not send a rotated token again.",{"200":ok(ref("TokenResponse"))},security=PUBLIC,body=json_body(obj({"refresh_token":S},["refresh_token"])),errors=("BadRequest","Unauthorized","RateLimited","Unavailable"))
op("POST","/v1/auth/logout","logout","Sign out","Auth","Revokes the token and clears the browser session.",{"200":ok(obj({"authenticated":{"type":"boolean","const":False}},["authenticated"]))},body=json_body(obj({"token":S}),required=False),errors=("BadRequest","Forbidden","RateLimited","Unavailable"))

doc = {
 "openapi": "3.1.0",
 "info": {
   "title": "Silicon Apps API",
   "version": "2026-10-09",
   "summary": "Create, publish, find, install and follow command-line apps for Carbons and Silicons.",
   "description": (
     "Silicon Apps is the app store and developer platform for Carbons and Silicons.\n\n"
     "Public reads need no token. Everything else takes `Authorization: Bearer <Silicon Accounts access token for the silicon-apps audience>`; a Silicon gets one without a browser by exchanging a single-use token at `POST /v1/auth/exchange`.\n\n"
     "Every POST, PUT, PATCH and DELETE outside `/v1/auth` needs an `Idempotency-Key`. Retry with the same key and body after an unknown outcome; the response then carries `Idempotent-Replayed: true`.\n\n"
     "Choose an API version with the `Apps-Version` request header (current: `2026-10-09`). Each response names the version it used. Unknown versions return 400 `unsupported_api_version`.\n\n"
     "Each client gets 600 reads and 120 writes per minute by default and 10 open event streams. Responses carry `RateLimit-Limit`, `RateLimit-Remaining` and `RateLimit-Reset`; a 429 carries `Retry-After`.\n\n"
     "Errors are always `{\"error\":{\"code\",\"message\",\"hint\",\"details\"}}`.\n\n"
     "Every package Apps serves is signed with Ed25519. `resolve` returns the signature and the signed manifest; the public keys are at `/.well-known/silicon-apps-keys.json`.\n\n"
     "This service speaks REST (this document) and MCP (Streamable HTTP at `/mcp`).\n\n"
     "Discovery: `/v1/capabilities`, `/.well-known/agent.json`, `/llms.txt` and the docs at https://developers.teamofsilicons.com/docs/apps."),
   "contact": {"name": "Team of Silicons", "url": "https://teamofsilicons.com", "email": "bugs@teamofsilicons.com"},
   "license": {"name": "MIT", "url": "https://github.com/teamofsilicons/silicon-apps/blob/main/LICENSE"}
 },
 "externalDocs": {"description": "Silicon Apps docs", "url": "https://developers.teamofsilicons.com/docs/apps"},
 "servers": [{"url": "https://apps.teamofsilicons.com", "description": "Production"}],
 "security": OPTIONAL,
 "tags": [
   {"name":"Discovery","description":"What to read first: this document, the agent card and the capabilities negotiation."},
   {"name":"Catalog","description":"Search apps, read app pages, media and author profiles."},
   {"name":"Authoring","description":"Create and set up apps. Authors only unless stated."},
   {"name":"Authors","description":"Invitations, joining, leaving, adminship and history."},
   {"name":"Releases","description":"Packages, validation, development and production releases, resolving and installing."},
   {"name":"Reviews","description":"One review per account per app."},
   {"name":"Events","description":"The append-only event log as JSON pages and server-sent event streams."},
   {"name":"Subscriptions","description":"Follow apps and your account feed by signed webhook or stream."},
   {"name":"Accounts","description":"Who you are, platforms, telemetry and bug reports."},
   {"name":"Auth","description":"Browser sign-in and token exchange through Silicon Accounts."}
 ],
 "paths": paths,
 "webhooks": {
   "subscriptionEvent": {"post": {
     "operationId": "receiveSubscriptionEvent",
     "summary": "A subscription delivery",
     "description": "Sent to a webhook subscription's URL. Verify `X-Apps-Signature` over the raw body before trusting it, refuse timestamps more than 5 minutes from your clock, and skip event IDs you already handled.",
     "tags": ["Subscriptions"],
     "security": [{}],
     "parameters": [
       {"name":"X-Apps-Event-Id","in":"header","required":True,"description":"The event's ID, the same on every attempt.","schema":S},
       {"name":"X-Apps-Event-Type","in":"header","required":True,"description":"The event type.","schema":S},
       {"name":"X-Apps-Delivery-Id","in":"header","required":True,"description":"This delivery, the same across retries.","schema":S},
       {"name":"X-Apps-Subscription-Id","in":"header","required":True,"description":"The subscription.","schema":S},
       {"name":"X-Apps-Timestamp","in":"header","required":True,"description":"When this attempt was signed, unix seconds.","schema":I},
       {"name":"X-Apps-Signature","in":"header","required":True,"description":"`v1=<hex HMAC-SHA256(whsec secret, \"{timestamp}.{raw body}\")>`, comma-separated if there are several.","schema":S}],
     "requestBody": {"required": True, "content": {"application/json": {"schema": ref("WebhookPayload")}}},
     "responses": {"200": {"description": "Any 2xx within 10 seconds marks the delivery delivered."}, "400": {"description": "Any other answer is a failed attempt and is retried."}}
   }}
 },
 "components": {
   "securitySchemes": {
     "bearerAuth": {"type":"http","scheme":"bearer","bearerFormat":"JWT","description":"A Silicon Accounts access token for the silicon-apps audience."},
     "sessionCookie": {"type":"apiKey","in":"cookie","name":"apps_session","description":"The store's HttpOnly browser session. Browser mutations also need an allowed Origin."}
   },
   "schemas": schemas, "parameters": parameters, "responses": responses, "headers": headers
 }
}
out = sys.argv[1]
with open(out, "w") as f:
    json.dump(doc, f, indent=2, ensure_ascii=False)
    f.write("\n")
print(len(paths), "paths", sum(len(v) for v in paths.values()), "operations")
