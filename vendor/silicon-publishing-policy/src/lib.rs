//! Organization ownership never depends on the member who created a resource.
//! Context comes from Organizations using a live, audience-bound Accounts token.
//! Never deserialize a caller-supplied context as authorization.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Apps,
    Interfaces,
    Cells,
}

impl ResourceKind {
    pub const ALL: [Self; 3] = [Self::Apps, Self::Interfaces, Self::Cells];
    pub fn app_id(self) -> &'static str {
        match self {
            Self::Apps => "silicon-apps",
            Self::Interfaces => "silicon-interfaces",
            Self::Cells => "silicon-cells",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Create,
    Read,
    Edit,
    Publish,
    Manage,
}

impl Permission {
    pub const ALL: [Self; 5] = [
        Self::Create,
        Self::Read,
        Self::Edit,
        Self::Publish,
        Self::Manage,
    ];
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Subject {
    Account(String),
    Team(String),
}

impl Subject {
    pub fn matches(&self, account: &str, teams: &BTreeSet<String>) -> bool {
        match self {
            Self::Account(id) => id == account,
            Self::Team(id) => teams.contains(id),
        }
    }
    fn validate(&self) -> Result<(), String> {
        let id = match self {
            Self::Account(id) | Self::Team(id) => id,
        };
        if id.is_empty()
            || id.len() > 100
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("Use permanent account UUIDs or team IDs for permission subjects.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub subject: Option<Subject>,
    /// Empty means every resource kind; a resource grant cannot specify kinds.
    #[serde(default)]
    pub resources: BTreeSet<ResourceKind>,
    pub permissions: BTreeSet<Permission>,
}

impl Grant {
    fn validate(&self, resource: bool) -> Result<(), String> {
        self.subject
            .as_ref()
            .ok_or("Every grant needs an account or team.")?
            .validate()?;
        if self.permissions.is_empty()
            || (resource
                && (!self.resources.is_empty() || self.permissions.contains(&Permission::Create)))
        {
            return Err("Resource grants need read, edit, publish or manage permissions; creation belongs in organization settings.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingPolicy {
    #[serde(default)]
    pub grants: Vec<Grant>,
}

impl PublishingPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.grants.len() > 500 {
            return Err("Use at most 500 publishing grants.".into());
        }
        for grant in &self.grants {
            grant.validate(false)?;
        }
        Ok(())
    }
    pub fn permissions(
        &self,
        account: &str,
        teams: &BTreeSet<String>,
        kind: ResourceKind,
    ) -> BTreeSet<Permission> {
        self.grants
            .iter()
            .filter(|g| {
                (g.resources.is_empty() || g.resources.contains(&kind))
                    && g.subject
                        .as_ref()
                        .is_some_and(|s| s.matches(account, teams))
            })
            .flat_map(|g| g.permissions.iter().copied())
            .collect()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudienceMode {
    #[default]
    Organization,
    Restricted,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Audience {
    #[serde(default)]
    pub mode: AudienceMode,
    #[serde(default)]
    pub include: BTreeSet<Subject>,
    #[serde(default)]
    pub exclude: BTreeSet<Subject>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourcePolicy {
    #[serde(default)]
    pub grants: Vec<Grant>,
    #[serde(default)]
    pub audience: Audience,
}

impl ResourcePolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.grants.len() > 500
            || self.audience.include.len() > 500
            || self.audience.exclude.len() > 500
        {
            return Err("Each access list is limited to 500 entries.".into());
        }
        for grant in &self.grants {
            grant.validate(true)?;
        }
        for subject in self.audience.include.iter().chain(&self.audience.exclude) {
            subject.validate()?;
        }
        Ok(())
    }
    /// Draft creators can edit their own resource while they remain members.
    /// Creating a draft never implicitly grants publication or access management.
    pub fn for_creator(account: &str) -> Self {
        Self {
            grants: vec![Grant {
                subject: Some(Subject::Account(account.into())),
                permissions: BTreeSet::from([Permission::Read, Permission::Edit]),
                ..Grant::default()
            }],
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OrganizationContext {
    pub org_id: String,
    pub name: String,
    pub logo_url: Option<String>,
    pub admin: bool,
    #[serde(default)]
    pub teams: BTreeSet<String>,
    /// Missing kinds are denied by the organization's app access policy.
    #[serde(default)]
    pub permissions: BTreeMap<ResourceKind, BTreeSet<Permission>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Context {
    pub account_uuid: String,
    #[serde(default)]
    pub organizations: Vec<OrganizationContext>,
}

pub fn valid_organization_id(id: &str) -> bool {
    (3..=30).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

#[derive(Deserialize)]
pub struct Publisher {
    pub org_id: String,
    pub name: String,
}

fn integration_base<'a>(base: &'a str, audience: &str) -> Result<&'a str, String> {
    let url = reqwest::Url::parse(base).map_err(|_| "Invalid Organizations URL.")?;
    if (url.scheme() != "https"
        && !(url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"))))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Organizations requires HTTPS (HTTP is allowed only on loopback).".into());
    }
    if audience.is_empty()
        || !audience
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    {
        return Err("Invalid integration audience.".into());
    }
    Ok(base.trim_end_matches('/'))
}

pub async fn lookup_publisher(
    base: &str,
    audience: &str,
    token: &str,
    id: &str,
) -> Result<Option<Publisher>, String> {
    if !valid_organization_id(id) {
        return Ok(None);
    }
    let base = integration_base(base, audience)?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| "Organizations is unavailable.")?;
    let mut response = client
        .get(format!("{base}/v1/integrations/{audience}/publishers/{id}"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| "Organizations is unavailable.")?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err("Organizations could not verify this publisher.".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Organizations response was interrupted.")?
    {
        if bytes.len() + chunk.len() > 16_384 {
            return Err("Organization profile is too large.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let publisher: Publisher =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid organization profile.")?;
    if publisher.org_id != id {
        return Err("Organizations returned a different publisher.".into());
    }
    Ok(Some(publisher))
}

impl Context {
    /// The verified work handle determines its home org. The create operation still checks its live grant.
    pub fn default_publisher<'a>(&'a self, account_id: &'a str) -> Result<Option<&'a str>, String> {
        if let Some((_, org)) = account_id.rsplit_once('.')
            && valid_organization_id(org)
        {
            return Ok(Some(org));
        }
        match self.organizations.as_slice() {
            [] => Ok(None),
            [org] => Ok(Some(&org.org_id)),
            _ => Err("Choose a publisher for this resource.".into()),
        }
    }
    /// Responding on behalf of an organization requires current management permission for the product.
    pub fn can_answer(&self, org: &str, kind: ResourceKind) -> bool {
        self.allows(org, kind, &ResourcePolicy::default(), Permission::Manage)
    }
    pub fn invitation_targets(&self, kind: ResourceKind) -> Vec<String> {
        self.organizations
            .iter()
            .filter(|org| self.can_answer(&org.org_id, kind))
            .map(|org| format!("org:{}", org.org_id))
            .collect()
    }
    /// A co-author can delegate author work through its own grants. Ownership and access administration stay with the host.
    pub fn author_allows(
        &self,
        authors: &[String],
        kind: ResourceKind,
        permission: Permission,
    ) -> bool {
        matches!(
            permission,
            Permission::Read | Permission::Edit | Permission::Publish
        ) && authors
            .iter()
            .any(|org| self.allows(org, kind, &ResourcePolicy::default(), permission))
    }

    pub fn organization(&self, org: &str) -> Option<&OrganizationContext> {
        self.organizations.iter().find(|o| o.org_id == org)
    }
    pub fn allows(
        &self,
        org: &str,
        kind: ResourceKind,
        policy: &ResourcePolicy,
        permission: Permission,
    ) -> bool {
        let Some(member) = self.organization(org) else {
            return false;
        };
        let Some(base) = member.permissions.get(&kind) else {
            return false;
        };
        if member.admin {
            return true;
        }
        let mut permissions = base.clone();
        if permission != Permission::Create {
            for grant in &policy.grants {
                if grant
                    .subject
                    .as_ref()
                    .is_some_and(|s| s.matches(&self.account_uuid, &member.teams))
                {
                    permissions.extend(grant.permissions.iter().copied());
                }
            }
        }
        // Manage includes resource operations, but does not implicitly permit
        // creating new resources. Edit and publish permit reading their resource.
        permissions.contains(&permission)
            || (permission != Permission::Create && permissions.contains(&Permission::Manage))
            || (permission == Permission::Read
                && permissions
                    .iter()
                    .any(|p| matches!(p, Permission::Edit | Permission::Publish)))
    }
    pub fn audience_allows(&self, org: &str, kind: ResourceKind, policy: &ResourcePolicy) -> bool {
        let Some(member) = self
            .organization(org)
            .filter(|o| o.permissions.contains_key(&kind))
        else {
            return false;
        };
        let matches = |s: &Subject| s.matches(&self.account_uuid, &member.teams);
        !policy.audience.exclude.iter().any(matches)
            && (policy.audience.mode == AudienceMode::Organization
                || policy.audience.include.iter().any(matches))
    }
}

/// Fetch per request, never from a browser-supplied group list. Failed verification
/// must not grant organization access; callers may continue personal operations.
pub async fn fetch(
    base: &str,
    audience: &str,
    token: &str,
    account: &str,
) -> Result<Context, String> {
    let base = integration_base(base, audience)?;
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| "Organizations is unavailable.")?;
    let response = http
        .get(format!(
            "{}/v1/integrations/{audience}/publishing",
            base.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| "Organizations is unavailable.")?;
    if !response.status().is_success() {
        return Err(
            "Organizations could not verify current membership and publishing permissions.".into(),
        );
    }
    if response.content_length().is_some_and(|n| n > 2_000_000) {
        return Err("Organizations context is too large.".into());
    }
    let context = read_context(response, account).await?;
    if context.account_uuid != account {
        return Err("Organizations returned a different account.".into());
    }
    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn member() -> Context {
        Context {
            account_uuid: "alice".into(),
            organizations: vec![OrganizationContext {
                org_id: "example".into(),
                teams: BTreeSet::from(["design".into()]),
                permissions: BTreeMap::from([(
                    ResourceKind::Apps,
                    BTreeSet::from([Permission::Create]),
                )]),
                ..OrganizationContext::default()
            }],
        }
    }
    #[test]
    fn organization_authority_is_live_product_specific_and_bounded() {
        let mut c = member();
        let authors = vec!["example".to_string()];
        let permissions = &mut c.organizations[0].permissions;
        permissions.insert(ResourceKind::Apps, BTreeSet::from([Permission::Manage]));
        permissions.insert(ResourceKind::Cells, BTreeSet::from([Permission::Edit]));
        assert_eq!(c.invitation_targets(ResourceKind::Apps), ["org:example"]);
        assert!(c.invitation_targets(ResourceKind::Cells).is_empty());
        for permission in [Permission::Read, Permission::Edit, Permission::Publish] {
            assert!(c.author_allows(&authors, ResourceKind::Apps, permission));
        }
        for permission in [Permission::Create, Permission::Manage] {
            assert!(!c.author_allows(&authors, ResourceKind::Apps, permission));
        }
        assert!(c.author_allows(&authors, ResourceKind::Cells, Permission::Read));
        assert!(!c.author_allows(&authors, ResourceKind::Cells, Permission::Publish));
        assert!(!c.author_allows(&authors, ResourceKind::Interfaces, Permission::Read));
        c.organizations.clear();
        assert!(c.invitation_targets(ResourceKind::Apps).is_empty());
        assert!(!c.author_allows(&authors, ResourceKind::Apps, Permission::Read));
    }

    #[test]
    fn work_account_publisher_is_stable_even_when_access_is_missing_or_ambiguous() {
        let mut c = Context::default();
        assert_eq!(c.default_publisher("c:personal").unwrap(), None);
        assert_eq!(
            c.default_publisher("si:helper.studio").unwrap(),
            Some("studio")
        );
        c.organizations = member().organizations;
        assert_eq!(c.default_publisher("c:legacy").unwrap(), Some("example"));
        c.organizations.push(OrganizationContext {
            org_id: "other".into(),
            ..Default::default()
        });
        assert!(c.default_publisher("c:legacy").is_err());
        assert_eq!(
            c.default_publisher("c:person.other").unwrap(),
            Some("other")
        );
    }

    #[test]
    fn creator_is_not_publisher_and_removal_revokes_resource_grants() {
        let mut c = member();
        let p = ResourcePolicy::for_creator("alice");
        assert!(c.allows("example", ResourceKind::Apps, &p, Permission::Create));
        assert!(c.allows("example", ResourceKind::Apps, &p, Permission::Edit));
        assert!(!c.allows("example", ResourceKind::Apps, &p, Permission::Publish));
        assert!(!c.allows("example", ResourceKind::Apps, &p, Permission::Manage));
        assert!(!c.allows("example", ResourceKind::Cells, &p, Permission::Edit));
        c.organizations.clear();
        assert!(!c.allows("example", ResourceKind::Apps, &p, Permission::Edit));
    }
    #[test]
    fn exclusions_win_and_team_access_ends_immediately() {
        let mut c = member();
        let mut p = ResourcePolicy::default();
        assert!(c.audience_allows("example", ResourceKind::Apps, &p));
        p.audience.mode = AudienceMode::Restricted;
        assert!(!c.audience_allows("example", ResourceKind::Apps, &p));
        p.audience.include.insert(Subject::Team("design".into()));
        assert!(c.audience_allows("example", ResourceKind::Apps, &p));
        p.audience.exclude.insert(Subject::Account("alice".into()));
        assert!(!c.audience_allows("example", ResourceKind::Apps, &p));
        p.audience.exclude.clear();
        c.organizations[0].teams.clear();
        assert!(!c.audience_allows("example", ResourceKind::Apps, &p));
    }
    #[test]
    fn per_resource_grants_do_not_grant_another_resource_or_creation() {
        let c = member();
        let mut p = ResourcePolicy::default();
        p.grants.push(Grant {
            subject: Some(Subject::Team("design".into())),
            permissions: BTreeSet::from([Permission::Publish]),
            ..Grant::default()
        });
        assert!(c.allows("example", ResourceKind::Apps, &p, Permission::Publish));
        assert!(!c.allows(
            "example",
            ResourceKind::Apps,
            &ResourcePolicy::default(),
            Permission::Publish
        ));
        p.grants[0].permissions.insert(Permission::Create);
        assert!(p.validate().is_err());
    }
}

async fn read_context(mut response: reqwest::Response, account: &str) -> Result<Context, String> {
    if !response.status().is_success() {
        return Err("Organizations could not verify current access.".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Organizations response was interrupted.")?
    {
        if bytes.len() + chunk.len() > 2_000_000 {
            return Err("Organizations context is too large.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let context: Context = serde_json::from_slice(&bytes)
        .map_err(|_| "Organizations returned invalid permissions.")?;
    if context.account_uuid != account {
        return Err("Organizations returned a different account.".into());
    }
    Ok(context)
}
/// Service-only authorization recheck. Never expose an app secret to a browser.
pub async fn fetch_service(
    base: &str,
    service: &str,
    secret: &str,
    account: &str,
) -> Result<Context, String> {
    let url = reqwest::Url::parse(base).map_err(|_| "Invalid Organizations URL.")?;
    if !(url.scheme() == "https"
        || (url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"))))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Invalid Organizations URL.".into());
    }
    if !["silicon-apps", "silicon-interfaces", "silicon-cells"].contains(&service)
        || account.is_empty()
        || !account
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
    {
        return Err("Invalid service publishing request.".into());
    }
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Organizations is unavailable.")?;
    let response = http
        .get(format!(
            "{}/v1/integrations/{service}/publishing/accounts/{account}",
            base.trim_end_matches('/')
        ))
        .basic_auth(service, Some(secret))
        .send()
        .await
        .map_err(|_| "Organizations is unavailable.")?;
    read_context(response, account).await
}
