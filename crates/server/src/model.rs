use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const APP_ID: &str = "silicon-apps";

pub const TARGETS: [&str; 9] = [
    "linux-x86_64",
    "linux-i686",
    "linux-aarch64",
    "linux-armv7hf",
    "windows-x86_64",
    "windows-i686",
    "windows-aarch64",
    "macos-x86_64",
    "macos-aarch64",
];
pub const COMMANDS: [&str; 3] = ["--help", "accounts --json", "login status --json"];
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Identity {
    pub uuid: String,
    pub id: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub verified_emails: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Author {
    pub uuid: String,
    pub id: String,
    pub display_name: String,
    pub joined_at: String,
}
impl Author {
    pub fn from_identity(i: &Identity) -> Self {
        Self {
            uuid: i.uuid.clone(),
            id: i.id.clone(),
            display_name: i.display_name.clone(),
            joined_at: now(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Review {
    pub uuid: String,
    pub id: String,
    pub rating: u8,
    pub text: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Invite {
    pub id: String,
    pub app_id: String,
    pub to: String,
    pub account_uuid: Option<String>,
    pub status: String,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Release {
    pub id: String,
    pub app_id: String,
    pub channel: String,
    pub version: String,
    pub package_ids: Vec<String>,
    pub notes: String,
    pub created_at: String,
    pub promoted_from: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Package {
    pub id: String,
    pub target: String,
    pub sha256: String,
    pub size: u64,
    pub command: String,
    pub validation: Vec<Value>,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct History {
    pub id: String,
    pub at: String,
    pub actor_uuid: String,
    pub kind: String,
    pub data: Value,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct App {
    pub app_id: String,
    pub name: String,
    pub description: String,
    pub logo: String,
    #[serde(default)]
    pub logo_alt: String,
    pub banner: String,
    #[serde(default)]
    pub banner_alt: String,
    pub tags: Vec<String>,
    pub visibility: String,
    pub domains: Vec<String>,
    pub account_ids: Vec<String>,
    pub access_uuids: Vec<String>,
    pub links: Value,
    pub carousel: Vec<Value>,
    pub published: bool,
    pub setup_step: u8,
    pub created_at: String,
    pub updated_at: String,
    pub authors: Vec<Author>,
    pub admin_uuid: String,
    pub packages: Vec<Package>,
    pub releases: Vec<Release>,
    pub reviews: Vec<Review>,
    pub installs: u64,
    pub history: Vec<History>,
    pub secret_hash: String,
}
impl App {
    pub fn is_author(&self, who: Option<&Identity>) -> bool {
        who.is_some_and(|i| self.authors.iter().any(|a| a.uuid == i.uuid))
    }
    pub fn is_admin(&self, who: Option<&Identity>) -> bool {
        who.is_some_and(|i| i.uuid == self.admin_uuid)
    }
    pub fn visible(&self, who: Option<&Identity>) -> bool {
        if self.is_author(who) {
            return true;
        }
        if !self.published {
            return false;
        }
        if self.visibility == "public" {
            return true;
        }
        who.is_some_and(|i| {
            self.access_uuids.contains(&i.uuid)
                || i.verified_emails.iter().any(|email| {
                    email.rsplit_once('@').is_some_and(|(_, d)| {
                        self.domains
                            .iter()
                            .any(|allowed| allowed.eq_ignore_ascii_case(d))
                    })
                })
        })
    }
    pub fn latest(&self, channel: &str) -> Option<&Release> {
        self.releases
            .iter()
            .filter(|r| r.channel == channel)
            .max_by_key(|r| version_tuple(&r.version).unwrap_or_default())
    }
    pub fn rating(&self) -> Option<f64> {
        if self.reviews.is_empty() {
            None
        } else {
            Some(
                self.reviews.iter().map(|r| r.rating as f64).sum::<f64>()
                    / self.reviews.len() as f64,
            )
        }
    }
    pub fn view(&self, who: Option<&Identity>) -> Value {
        let mut targets: Vec<_> = self
            .latest("production")
            .or_else(|| self.latest("development"))
            .map(|r| {
                self.packages
                    .iter()
                    .filter(|p| r.package_ids.contains(&p.id))
                    .map(|p| p.target.clone())
                    .collect()
            })
            .unwrap_or_default();
        targets.sort();
        let mut v = json!({"app_id":self.app_id,"name":self.name,"description":self.description,"logo":self.logo,"banner":self.banner,"logo_alt":self.logo_alt,"banner_alt":self.banner_alt,"tags":self.tags,"visibility":self.visibility,"links":self.links,"carousel":self.carousel,"published":self.published,"setup_step":self.setup_step,"created_at":self.created_at,"updated_at":self.updated_at,"authors":self.authors,"targets":targets,"latest_production":self.latest("production"),"latest_development":self.latest("development"),"rating":self.rating(),"review_count":self.reviews.len(),"installs":self.installs,"is_author":self.is_author(who),"is_admin":self.is_admin(who)});
        if self.is_author(who) {
            v["domains"] = json!(self.domains);
            v["account_ids"] = json!(self.account_ids);
        }
        v
    }
    pub fn event(&mut self, who: &str, kind: &str, data: Value) {
        self.updated_at = now();
        self.history.push(History {
            id: new_id(),
            at: now(),
            actor_uuid: who.into(),
            kind: kind.into(),
            data,
            idempotency_key: None,
        });
    }
    pub fn readiness(&self) -> Value {
        let mut errors = vec![];
        if self.name.trim().is_empty() {
            errors.push(json!({"field":"name","message":"App name is required."}));
        }
        if !(200..=600).contains(&self.description.chars().count()) {
            errors.push(json!({"field":"description","message":"Description must contain 200 to 600 characters."}));
        }
        if self.releases.is_empty() {
            errors.push(json!({"field":"packages","message":"Create a development release containing at least one validated package."}));
        }
        json!({"ready":errors.is_empty(),"errors":errors,"required_commands":COMMANDS})
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub apps: BTreeMap<String, App>,
    pub invites: Vec<Invite>,
    pub reports: Vec<Value>,
    #[serde(default)]
    pub platforms: BTreeMap<String, Vec<String>>,
}
pub fn valid_app_id(s: &str) -> bool {
    (3..=30).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}
pub fn reserved_app_id(s: &str) -> bool {
    matches!(s, "accounts" | "developer" | "apps")
}
pub fn version_tuple(s: &str) -> Option<(u64, u64, u64)> {
    let p: Vec<_> = s.split('.').collect();
    if p.len() != 3
        || p.iter().any(|p| {
            p.is_empty()
                || !p.bytes().all(|b| b.is_ascii_digit())
                || (p.len() > 1 && p.starts_with('0'))
        })
    {
        return None;
    }
    Some((p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?))
}
