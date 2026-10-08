//! Everything a signed-in Carbon or Silicon can do with its first-party access token.

use std::fmt;

use bytes::Bytes;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::app::AppClient;
use crate::client::{AccountsClient, Auth, Request, Response};
use crate::error::{Error, Result};
use crate::secret::Secret;
use crate::serde_util::unwrap_key;
use crate::types::{
    AccountKind, AccountSummary, Contact, ContactChallenge, CreateSilicon, CustodianRequest,
    DeliveriesQuery, DeliveryDetail, DeviceRequest, EmailAddress, HistoryItem, HistoryQuery,
    IdAvailability, Identity, ManagedSilicon, Me, MyApp, MyProof, OwnedApp, Page, PhoneNumber,
    PhotoUploaded, ProfileUpdate, ReplayRequest, ReplayResult, SessionInfo, ShortLivedToken,
    SiliconCreated, SiliconPhotoUploaded, SiliconView, SiliconWebhook, StkRotated, UpdateSilicon,
    WebhookDelivery, WebhookTestResult,
};

/// A signed-in Carbon or Silicon, authenticated with a first-party access token
/// (`aud = accounts`). Created with [`AccountsClient::with_token`]. Holds no state
/// besides the token: refreshing it is up to the caller
/// ([`AccountsClient::refresh_first_party`]).
pub struct AccountSession<'a> {
    client: &'a AccountsClient,
    token: Secret,
}

impl fmt::Debug for AccountSession<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountSession")
            .field("base_url", &self.client.base_url().as_str())
            .field("token", &self.token)
            .finish()
    }
}

impl<'a> AccountSession<'a> {
    pub(crate) fn new(client: &'a AccountsClient, token: String) -> Self {
        Self {
            client,
            token: Secret::new(token),
        }
    }

    /// The client this session uses.
    pub fn client(&self) -> &'a AccountsClient {
        self.client
    }

    /// The access token.
    pub fn access_token(&self) -> &str {
        self.token.expose()
    }

    fn auth(&self) -> Auth<'_> {
        Auth::Bearer(self.token.expose())
    }

    fn url(&self, segments: &[&str]) -> url::Url {
        self.client.endpoint(segments)
    }

    async fn get<T: DeserializeOwned>(&self, segments: &[&str]) -> Result<T> {
        self.client.get(self.url(segments), self.auth()).await
    }

    async fn page<T: DeserializeOwned>(&self, url: url::Url) -> Result<Page<T>> {
        self.client.get_page(url, self.auth()).await
    }

    /// Every item of a paginated list, following `next_cursor` (at most 100 pages).
    async fn all<T: DeserializeOwned>(&self, segments: &[&str]) -> Result<Vec<T>> {
        let mut items = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..100 {
            let url = self.client.endpoint_with_query(
                segments,
                &[("limit", Some("200".to_owned())), ("cursor", cursor.take())],
            );
            let page: Page<T> = self.page(url).await?;
            items.extend(page.items);
            match page.next_cursor {
                Some(next) if !next.is_empty() => cursor = Some(next),
                _ => break,
            }
        }
        Ok(items)
    }

    async fn send(
        &self,
        method: Method,
        segments: &[&str],
        body: Option<&Value>,
    ) -> Result<Response> {
        let mut request = Request::new(method, self.url(segments), self.auth());
        if let Some(body) = body {
            request = request.json(body)?;
        }
        self.client.execute(request).await
    }

    // ---- profile ------------------------------------------------------------------------

    /// `GET /v1/me`: the full account.
    pub async fn me(&self) -> Result<Me> {
        self.get(&["v1", "me"]).await
    }

    /// `PATCH /v1/me`: change display name, timezone, dob (Carbons) or photo URL. Apps that
    /// can see a changed field get `account.updated`.
    pub async fn update_me(&self, update: &ProfileUpdate) -> Result<Me> {
        if update.is_empty() {
            return Err(Error::invalid_input(
                "Nothing to update: no display name, timezone, date of birth or photo URL was given.",
                "Pass at least one field to change.",
            ));
        }
        let body = serde_json::to_value(update).unwrap_or(Value::Null);
        let response = self.send(Method::PATCH, &["v1", "me"], Some(&body)).await?;
        self.me_or_refetch(&response).await
    }

    /// `POST /v1/me/id`: change your own `c:`/`si:` id. The old id stays reserved for you
    /// for 10 days; every app you signed into gets `account.id_changed`.
    pub async fn change_id(&self, new_id: &str) -> Result<Me> {
        let body = json!({ "id": new_id.trim() });
        let response = self
            .send(Method::POST, &["v1", "me", "id"], Some(&body))
            .await?;
        self.me_or_refetch(&response).await
    }

    /// `GET /v1/ids/available` as the signed-in account: an id reserved for you after a
    /// change is reported `available: true, reclaimable: true`.
    pub async fn id_available(&self, id: &str) -> Result<IdAvailability> {
        let url = self.client.endpoint_with_query(
            &["v1", "ids", "available"],
            &[("id", Some(id.trim().to_owned()))],
        );
        self.client.get(url, self.auth()).await
    }

    /// `GET /v1/ids/available?id=…&for=…` as a custodian: whether `id` can be taken by the
    /// Silicon `silicon` (its uuid or si:id) you are custodian of. An id reserved for that
    /// Silicon (one of its recent ids) is `available: true, reclaimable: true`.
    pub async fn silicon_id_available(&self, silicon: &str, id: &str) -> Result<IdAvailability> {
        let url = self.client.endpoint_with_query(
            &["v1", "ids", "available"],
            &[
                ("id", Some(id.trim().to_owned())),
                ("for", Some(silicon.trim().to_owned())),
            ],
        );
        self.client.get(url, self.auth()).await
    }

    /// `POST /v1/me/photo`: upload a profile photo (png, jpeg, webp or gif, at most 2 MB).
    pub async fn set_photo(
        &self,
        bytes: impl Into<Bytes>,
        content_type: &str,
    ) -> Result<PhotoUploaded> {
        let bytes = check_photo(bytes.into(), content_type)?;
        let request = Request::new(Method::POST, self.url(&["v1", "me", "photo"]), self.auth())
            .raw(bytes, content_type);
        self.client.execute(request).await?.json()
    }

    /// `DELETE /v1/me/photo`: back to the default profile photo. Returns the account.
    pub async fn remove_photo(&self) -> Result<Me> {
        let response = self
            .send(Method::DELETE, &["v1", "me", "photo"], None)
            .await?;
        self.me_or_refetch(&response).await
    }

    async fn me_or_refetch(&self, response: &Response) -> Result<Me> {
        let value = unwrap_key(response.value()?, "account");
        match serde_json::from_value::<Me>(value) {
            Ok(me) => Ok(me),
            Err(_) => self.me().await,
        }
    }

    // ---- emails and phones (Carbons) -----------------------------------------------------

    /// `GET /v1/me/emails`.
    pub async fn emails(&self) -> Result<Vec<EmailAddress>> {
        let response = self
            .send(Method::GET, &["v1", "me", "emails"], None)
            .await?;
        list_field(&response, "emails")
    }

    /// `POST /v1/me/emails`: sends a code to a new email address (up to 10 per account).
    pub async fn add_email(&self, email: &str) -> Result<ContactChallenge> {
        let body = json!({ "email": email.trim() });
        self.send(Method::POST, &["v1", "me", "emails"], Some(&body))
            .await?
            .json()
    }

    /// `POST /v1/me/emails/verify`: proves the code and adds the email.
    pub async fn verify_email(&self, challenge_id: &str, code: &str) -> Result<Vec<EmailAddress>> {
        let body = json!({ "challenge_id": challenge_id, "code": code.trim() });
        let response = self
            .send(Method::POST, &["v1", "me", "emails", "verify"], Some(&body))
            .await?;
        self.emails_or_refetch(&response).await
    }

    /// `POST /v1/me/emails/{email}/primary`.
    pub async fn make_email_primary(&self, email: &str) -> Result<Vec<EmailAddress>> {
        let response = self
            .send(
                Method::POST,
                &["v1", "me", "emails", email.trim(), "primary"],
                None,
            )
            .await?;
        self.emails_or_refetch(&response).await
    }

    /// `DELETE /v1/me/emails/{email}`. The primary email can't be removed: make another
    /// one primary first.
    pub async fn remove_email(&self, email: &str) -> Result<Vec<EmailAddress>> {
        let response = self
            .send(Method::DELETE, &["v1", "me", "emails", email.trim()], None)
            .await?;
        self.emails_or_refetch(&response).await
    }

    async fn emails_or_refetch(&self, response: &Response) -> Result<Vec<EmailAddress>> {
        match list_field(response, "emails") {
            Ok(list) => Ok(list),
            Err(_) => self.emails().await,
        }
    }

    /// `GET /v1/me/phones`.
    pub async fn phones(&self) -> Result<Vec<PhoneNumber>> {
        let response = self
            .send(Method::GET, &["v1", "me", "phones"], None)
            .await?;
        list_field(&response, "phones")
    }

    /// `POST /v1/me/phones`: sends a code by SMS (up to 10 per account). `country` (ISO,
    /// e.g. `IN`) lets you pass a local number.
    pub async fn add_phone(&self, phone: &str, country: Option<&str>) -> Result<ContactChallenge> {
        let contact = Contact::Phone {
            phone: phone.trim().to_owned(),
            country: country.map(str::to_owned),
        };
        self.send(
            Method::POST,
            &["v1", "me", "phones"],
            Some(&contact.to_json()),
        )
        .await?
        .json()
    }

    /// `POST /v1/me/phones/verify`.
    pub async fn verify_phone(&self, challenge_id: &str, code: &str) -> Result<Vec<PhoneNumber>> {
        let body = json!({ "challenge_id": challenge_id, "code": code.trim() });
        let response = self
            .send(Method::POST, &["v1", "me", "phones", "verify"], Some(&body))
            .await?;
        self.phones_or_refetch(&response).await
    }

    /// `POST /v1/me/phones/{phone}/primary`.
    pub async fn make_phone_primary(&self, phone: &str) -> Result<Vec<PhoneNumber>> {
        let response = self
            .send(
                Method::POST,
                &["v1", "me", "phones", phone.trim(), "primary"],
                None,
            )
            .await?;
        self.phones_or_refetch(&response).await
    }

    /// `DELETE /v1/me/phones/{phone}`.
    pub async fn remove_phone(&self, phone: &str) -> Result<Vec<PhoneNumber>> {
        let response = self
            .send(Method::DELETE, &["v1", "me", "phones", phone.trim()], None)
            .await?;
        self.phones_or_refetch(&response).await
    }

    async fn phones_or_refetch(&self, response: &Response) -> Result<Vec<PhoneNumber>> {
        match list_field(response, "phones") {
            Ok(list) => Ok(list),
            Err(_) => self.phones().await,
        }
    }

    // ---- identities, apps, sessions, history --------------------------------------------

    /// `GET /v1/me/identities`: linked Google / Apple identities.
    pub async fn identities(&self) -> Result<Vec<Identity>> {
        let response = self
            .send(Method::GET, &["v1", "me", "identities"], None)
            .await?;
        list_field(&response, "identities")
    }

    /// `DELETE /v1/me/identities/{provider}/{subject}`.
    pub async fn remove_identity(&self, provider: &str, subject: &str) -> Result<()> {
        self.send(
            Method::DELETE,
            &["v1", "me", "identities", provider, subject],
            None,
        )
        .await
        .map(|_| ())
    }

    /// `GET /v1/me/apps`: apps this account signed into.
    pub async fn apps(&self) -> Result<Vec<MyApp>> {
        self.all(&["v1", "me", "apps"]).await
    }

    /// `DELETE /v1/me/apps/{app_id}`: removes the app's access (its tokens and the OBO
    /// proofs it issued about you are revoked; it gets `membership.access_removed`).
    pub async fn remove_app_access(&self, app_id: &str) -> Result<()> {
        self.send(Method::DELETE, &["v1", "me", "apps", app_id], None)
            .await
            .map(|_| ())
    }

    /// `GET /v1/me/sessions`: browser sessions and CLI sign-ins.
    pub async fn sessions(&self) -> Result<Vec<SessionInfo>> {
        self.all(&["v1", "me", "sessions"]).await
    }

    /// `DELETE /v1/me/sessions/{id}`.
    pub async fn revoke_session(&self, id: &str) -> Result<()> {
        self.send(Method::DELETE, &["v1", "me", "sessions", id], None)
            .await
            .map(|_| ())
    }

    /// `GET /v1/me/history`.
    pub async fn history(&self, query: &HistoryQuery) -> Result<Page<HistoryItem>> {
        let url = self.client.endpoint_with_query(
            &["v1", "me", "history"],
            &[
                ("kind", query.kind.clone()),
                ("limit", query.limit.map(|l| l.to_string())),
                ("cursor", query.cursor.clone()),
            ],
        );
        self.page(url).await
    }

    /// `DELETE /v1/me`: deletes the account. `confirm` must be your current id. Fails
    /// with `custodian_of_silicons` while you are custodian of any Silicon.
    pub async fn delete_account(&self, confirm: &str) -> Result<()> {
        let body = json!({ "confirm": confirm.trim() });
        self.send(Method::DELETE, &["v1", "me"], Some(&body))
            .await
            .map(|_| ())
    }

    /// Signs out: revokes the first-party token family of this refresh token.
    pub async fn signout(&self, refresh_token: &str) -> Result<()> {
        self.client.revoke_first_party(refresh_token).await
    }

    // ---- signing into apps --------------------------------------------------------------

    /// `POST /v1/me/short-lived-tokens`: a single-use token (`slt_…`, 2 minutes) the app
    /// exchanges for your tokens. This is how a Silicon signs into an app.
    pub async fn short_lived_token(&self, app_id: &str) -> Result<ShortLivedToken> {
        let body = json!({ "app_id": app_id.trim() });
        self.send(
            Method::POST,
            &["v1", "me", "short-lived-tokens"],
            Some(&body),
        )
        .await?
        .json()
    }

    /// `GET /v1/me/proofs`: OBO proofs apps issued on your behalf.
    pub async fn proofs(&self) -> Result<Vec<MyProof>> {
        self.all(&["v1", "me", "proofs"]).await
    }

    /// `DELETE /v1/me/proofs/{proof_id}`.
    pub async fn revoke_proof(&self, proof_id: &str) -> Result<()> {
        self.send(Method::DELETE, &["v1", "me", "proofs", proof_id], None)
            .await
            .map(|_| ())
    }

    // ---- Silicon self-service -----------------------------------------------------------

    /// `PUT /v1/me/webhook` (Silicon): set your webhook endpoint. The response holds the
    /// new signing secret, shown once.
    pub async fn set_my_webhook(&self, url: &str) -> Result<SiliconWebhook> {
        let body = json!({ "url": url.trim() });
        self.send(Method::PUT, &["v1", "me", "webhook"], Some(&body))
            .await?
            .json()
    }

    /// `DELETE /v1/me/webhook` (Silicon).
    pub async fn remove_my_webhook(&self) -> Result<()> {
        self.send(Method::DELETE, &["v1", "me", "webhook"], None)
            .await
            .map(|_| ())
    }

    /// `POST /v1/me/webhook/test` (Silicon): queues a `ping`.
    pub async fn test_my_webhook(&self) -> Result<WebhookTestResult> {
        let response = self
            .send(Method::POST, &["v1", "me", "webhook", "test"], None)
            .await?;
        Ok(serde_json::from_value(response.value()?).unwrap_or_default())
    }

    /// `GET /v1/me/webhook/deliveries` (Silicon): deliveries of your own webhook, newest
    /// first, filtered by `status` (`pending`, `delivered` or `failed`).
    pub async fn my_webhook_deliveries(
        &self,
        query: &DeliveriesQuery,
    ) -> Result<Page<WebhookDelivery>> {
        self.page(deliveries_url(
            self.client,
            &["v1", "me", "webhook", "deliveries"],
            query,
        ))
        .await
    }

    /// `GET /v1/me/webhook/deliveries/{id}` (Silicon): one delivery with its attempts and the
    /// exact payload that was signed.
    pub async fn my_webhook_delivery(&self, delivery_id: &str) -> Result<DeliveryDetail> {
        self.get(&["v1", "me", "webhook", "deliveries", delivery_id.trim()])
            .await
    }

    /// `POST /v1/me/webhook/replay` (Silicon): re-queues deliveries of your own webhook (at
    /// most 100 per call) with the same event ids, sent to your current URL and signed with
    /// your current secret. With [`ReplayRequest::Failed`], call again until `remaining` is 0.
    /// Pass an idempotency key so a retried call doesn't replay twice.
    pub async fn replay_my_webhook(
        &self,
        request: &ReplayRequest,
        idempotency_key: Option<&str>,
    ) -> Result<ReplayResult> {
        self.replay(&["v1", "me", "webhook", "replay"], request, idempotency_key)
            .await
    }

    async fn replay(
        &self,
        segments: &[&str],
        request: &ReplayRequest,
        idempotency_key: Option<&str>,
    ) -> Result<ReplayResult> {
        request.check()?;
        let body = request.to_json();
        let request = Request::new(Method::POST, self.url(segments), self.auth())
            .json(&body)?
            .idempotency_key(idempotency_key)?;
        let response = self.client.execute(request).await?;
        Ok(serde_json::from_value(response.value()?).unwrap_or_default())
    }

    // ---- custodian side (Carbons) -------------------------------------------------------

    /// `GET /v1/me/silicons`: Silicons you are custodian of.
    pub async fn silicons(&self) -> Result<Vec<ManagedSilicon>> {
        self.all(&["v1", "me", "silicons"]).await
    }

    /// `POST /v1/me/silicons`: create a Silicon with you as its custodian. The generated
    /// STK in the response is shown once.
    pub async fn create_silicon(
        &self,
        request: &CreateSilicon,
        idempotency_key: Option<&str>,
    ) -> Result<SiliconCreated> {
        self.client
            .send_json(
                Method::POST,
                self.url(&["v1", "me", "silicons"]),
                self.auth(),
                request,
                idempotency_key,
            )
            .await
    }

    /// `GET /v1/me/silicons/{uuid}`.
    pub async fn get_silicon(&self, uuid: &str) -> Result<ManagedSilicon> {
        let response = self
            .send(Method::GET, &["v1", "me", "silicons", uuid], None)
            .await?;
        response.json()
    }

    /// `PATCH /v1/me/silicons/{uuid}`.
    pub async fn update_silicon(&self, uuid: &str, update: &UpdateSilicon) -> Result<SiliconView> {
        if update.is_empty() {
            return Err(Error::invalid_input(
                "Nothing to update: no display name, timezone or photo URL was given.",
                "Pass at least one field to change.",
            ));
        }
        let body = serde_json::to_value(update).unwrap_or(Value::Null);
        let response = self
            .send(Method::PATCH, &["v1", "me", "silicons", uuid], Some(&body))
            .await?;
        self.silicon_or_refetch(&response, uuid).await
    }

    /// `POST /v1/me/silicons/{uuid}/photo`: upload a Silicon's profile photo as its
    /// custodian (png, jpeg, webp or gif, at most 2 MB; the photo belongs to the Silicon).
    /// Pass an idempotency key so a retried upload doesn't upload twice.
    pub async fn set_silicon_photo(
        &self,
        uuid: &str,
        bytes: impl Into<Bytes>,
        content_type: &str,
        idempotency_key: Option<&str>,
    ) -> Result<SiliconPhotoUploaded> {
        let bytes = check_photo(bytes.into(), content_type)?;
        let request = Request::new(
            Method::POST,
            self.url(&["v1", "me", "silicons", uuid, "photo"]),
            self.auth(),
        )
        .raw(bytes, content_type)
        .idempotency_key(idempotency_key)?;
        self.client.execute(request).await?.json()
    }

    /// `POST /v1/me/silicons/{uuid}/id`: change a Silicon's si:id.
    pub async fn change_silicon_id(&self, uuid: &str, new_id: &str) -> Result<SiliconView> {
        let body = json!({ "id": new_id.trim() });
        let response = self
            .send(
                Method::POST,
                &["v1", "me", "silicons", uuid, "id"],
                Some(&body),
            )
            .await?;
        self.silicon_or_refetch(&response, uuid).await
    }

    async fn silicon_or_refetch(&self, response: &Response, uuid: &str) -> Result<SiliconView> {
        let value = unwrap_key(response.value()?, "silicon");
        match serde_json::from_value::<Me>(value) {
            Ok(silicon) => Ok(silicon),
            Err(_) => Ok(self.get_silicon(uuid).await?.silicon),
        }
    }

    /// `POST /v1/me/silicons/{uuid}/stk`: rotate the STK. The old STK dies immediately and
    /// every token family of the Silicon is revoked. Pass `stk` to choose it, or `None`
    /// to generate one (returned once).
    pub async fn rotate_stk(&self, uuid: &str, stk: Option<&str>) -> Result<StkRotated> {
        let body = match stk {
            Some(stk) => json!({ "stk": stk.trim() }),
            None => json!({}),
        };
        self.send(
            Method::POST,
            &["v1", "me", "silicons", uuid, "stk"],
            Some(&body),
        )
        .await?
        .json()
    }

    /// `PUT /v1/me/silicons/{uuid}/webhook`.
    pub async fn set_silicon_webhook(&self, uuid: &str, url: &str) -> Result<SiliconWebhook> {
        let body = json!({ "url": url.trim() });
        self.send(
            Method::PUT,
            &["v1", "me", "silicons", uuid, "webhook"],
            Some(&body),
        )
        .await?
        .json()
    }

    /// `DELETE /v1/me/silicons/{uuid}/webhook`.
    pub async fn remove_silicon_webhook(&self, uuid: &str) -> Result<()> {
        self.send(
            Method::DELETE,
            &["v1", "me", "silicons", uuid, "webhook"],
            None,
        )
        .await
        .map(|_| ())
    }

    /// `GET /v1/me/silicons/{uuid}/webhook/deliveries` (custodian): deliveries of a Silicon's
    /// own webhook, newest first, filtered by `status`.
    pub async fn silicon_webhook_deliveries(
        &self,
        uuid: &str,
        query: &DeliveriesQuery,
    ) -> Result<Page<WebhookDelivery>> {
        self.page(deliveries_url(
            self.client,
            &["v1", "me", "silicons", uuid, "webhook", "deliveries"],
            query,
        ))
        .await
    }

    /// `GET /v1/me/silicons/{uuid}/webhook/deliveries/{id}` (custodian): one delivery of a
    /// Silicon's webhook with its attempts and payload.
    pub async fn silicon_webhook_delivery(
        &self,
        uuid: &str,
        delivery_id: &str,
    ) -> Result<DeliveryDetail> {
        self.get(&[
            "v1",
            "me",
            "silicons",
            uuid,
            "webhook",
            "deliveries",
            delivery_id.trim(),
        ])
        .await
    }

    /// `POST /v1/me/silicons/{uuid}/webhook/replay` (custodian): re-queues deliveries of a
    /// Silicon's webhook, exactly like [`AccountSession::replay_my_webhook`].
    pub async fn replay_silicon_webhook(
        &self,
        uuid: &str,
        request: &ReplayRequest,
        idempotency_key: Option<&str>,
    ) -> Result<ReplayResult> {
        self.replay(
            &["v1", "me", "silicons", uuid, "webhook", "replay"],
            request,
            idempotency_key,
        )
        .await
    }

    /// `POST /v1/me/silicons/{uuid}/transfer`: ask another Carbon (`c:` id or email) to
    /// become the custodian. Nothing changes until they accept (14 days).
    pub async fn transfer_silicon(&self, uuid: &str, to: &str) -> Result<CustodianRequest> {
        let body = json!({ "to": to.trim() });
        let response = self
            .send(
                Method::POST,
                &["v1", "me", "silicons", uuid, "transfer"],
                Some(&body),
            )
            .await?;
        response.json_from(unwrap_key(response.value()?, "request"))
    }

    /// `DELETE /v1/me/silicons/{uuid}/transfer`: cancel a pending transfer.
    pub async fn cancel_transfer(&self, uuid: &str) -> Result<()> {
        self.send(
            Method::DELETE,
            &["v1", "me", "silicons", uuid, "transfer"],
            None,
        )
        .await
        .map(|_| ())
    }

    /// `DELETE /v1/me/silicons/{uuid}`: delete a Silicon account. `confirm` must be its
    /// current si:id.
    pub async fn delete_silicon(&self, uuid: &str, confirm: &str) -> Result<()> {
        let body = json!({ "confirm": confirm.trim() });
        self.send(Method::DELETE, &["v1", "me", "silicons", uuid], Some(&body))
            .await
            .map(|_| ())
    }

    /// `GET /v1/me/custodian-requests`: requests waiting for you (initial or transfer).
    pub async fn custodian_requests(&self) -> Result<Vec<CustodianRequest>> {
        self.all(&["v1", "me", "custodian-requests"]).await
    }

    /// `POST /v1/me/custodian-requests/{id}/accept`.
    pub async fn accept_custodian_request(&self, id: &str) -> Result<()> {
        self.send(
            Method::POST,
            &["v1", "me", "custodian-requests", id, "accept"],
            None,
        )
        .await
        .map(|_| ())
    }

    /// `POST /v1/me/custodian-requests/{id}/decline`.
    pub async fn decline_custodian_request(&self, id: &str) -> Result<()> {
        self.send(
            Method::POST,
            &["v1", "me", "custodian-requests", id, "decline"],
            None,
        )
        .await
        .map(|_| ())
    }

    // ---- apps you own and device approvals ----------------------------------------------

    /// `GET /v1/me/owned-apps`: apps you own (created in Silicon Apps).
    pub async fn owned_apps(&self) -> Result<Vec<OwnedApp>> {
        self.all(&["v1", "me", "owned-apps"]).await
    }

    /// Manage an app you own with this session instead of the app's credentials. Calls
    /// that need the app's own secret (token exchange, proof issuing and verifying) are
    /// refused with a precise error in this mode.
    pub fn app(&self, app_id: impl Into<String>) -> AppClient<'a> {
        AppClient::as_owner(self.client, app_id.into(), self.token.expose().to_owned())
    }

    /// `GET /v1/device/{user_code}`: a pending CLI sign-in you are asked to approve.
    pub async fn device_request(&self, user_code: &str) -> Result<DeviceRequest> {
        let code = normalize_user_code(user_code);
        self.get(&["v1", "device", &code]).await
    }

    /// `POST /v1/device/{user_code}/approve`: approve a CLI sign-in as yourself.
    pub async fn approve_device(&self, user_code: &str) -> Result<()> {
        let code = normalize_user_code(user_code);
        self.send(Method::POST, &["v1", "device", &code, "approve"], None)
            .await
            .map(|_| ())
    }

    /// `POST /v1/device/{user_code}/deny`.
    pub async fn deny_device(&self, user_code: &str) -> Result<()> {
        let code = normalize_user_code(user_code);
        self.send(Method::POST, &["v1", "device", &code, "deny"], None)
            .await
            .map(|_| ())
    }

    // ---- lookup -------------------------------------------------------------------------

    /// `GET /v1/accounts/{uuid}`.
    pub async fn lookup(&self, uuid: &str) -> Result<AccountSummary> {
        self.get(&["v1", "accounts", uuid.trim()]).await
    }

    /// `GET /v1/accounts/by-id/{id}` (`c:saket`, `si:scout`; current ids only).
    pub async fn lookup_by_id(&self, id: &str) -> Result<AccountSummary> {
        self.get(&["v1", "accounts", "by-id", &id.trim().to_ascii_lowercase()])
            .await
    }

    /// Resolves a uuid or a `c:`/`si:` id to an account.
    pub async fn resolve(&self, uuid_or_id: &str) -> Result<AccountSummary> {
        if AccountKind::of_id(uuid_or_id).is_some() {
            self.lookup_by_id(uuid_or_id).await
        } else {
            self.lookup(uuid_or_id).await
        }
    }
}

/// `wdjb mjht` / `wdjbmjht` → `WDJB-MJHT`.
fn normalize_user_code(code: &str) -> String {
    let compact: String = code
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_uppercase();
    if compact.len() == 8 {
        format!("{}-{}", &compact[..4], &compact[4..])
    } else {
        code.trim().to_ascii_uppercase()
    }
}

/// Decodes a list from a bare array, `{"items": […]}` or `{"<field>": […]}`.
fn list_field<T: DeserializeOwned>(response: &Response, field: &str) -> Result<Vec<T>> {
    let value = response.value()?;
    let list = match value {
        Value::Array(_) => value,
        Value::Object(mut map) => match (map.remove(field), map.remove("items")) {
            (Some(list @ Value::Array(_)), _) | (None, Some(list @ Value::Array(_))) => list,
            _ => {
                return Err(Error::decode(
                    format!(
                        "Silicon Accounts answered {} without a `{field}` list.",
                        response.what
                    ),
                    "Make sure this client is up to date with the service.",
                ));
            }
        },
        _ => {
            return Err(Error::decode(
                format!(
                    "Silicon Accounts answered {} without a `{field}` list.",
                    response.what
                ),
                "Make sure this client is up to date with the service.",
            ));
        }
    };
    response.json_from(list)
}

/// Checks a photo upload before sending it: an accepted image type, at most 2 MB.
fn check_photo(bytes: Bytes, content_type: &str) -> Result<Bytes> {
    const ALLOWED: [&str; 4] = ["image/png", "image/jpeg", "image/webp", "image/gif"];
    if !ALLOWED.contains(&content_type) {
        return Err(Error::invalid_input(
            format!(
                "Profile photos must be PNG, JPEG, WebP or GIF; `{content_type}` is not supported."
            ),
            "Convert the image to one of those formats.",
        ));
    }
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(Error::invalid_input(
            format!(
                "The photo is {} bytes; profile photos are limited to 2 MB.",
                bytes.len()
            ),
            "Resize or compress the image below 2 MB.",
        ));
    }
    Ok(bytes)
}

/// A delivery list URL with its `status`, `limit` and `cursor`.
fn deliveries_url(client: &AccountsClient, segments: &[&str], query: &DeliveriesQuery) -> url::Url {
    client.endpoint_with_query(
        segments,
        &[
            ("status", query.status.clone()),
            ("limit", query.limit.map(|l| l.to_string())),
            ("cursor", query.cursor.clone()),
        ],
    )
}
#[cfg(test)]
mod tests {
    use super::normalize_user_code;

    #[test]
    fn user_codes_are_normalized() {
        assert_eq!(normalize_user_code("wdjb-mjht"), "WDJB-MJHT");
        assert_eq!(normalize_user_code("WDJBMJHT"), "WDJB-MJHT");
        assert_eq!(normalize_user_code(" wdjb mjht "), "WDJB-MJHT");
        assert_eq!(normalize_user_code("abc"), "ABC");
    }
}
