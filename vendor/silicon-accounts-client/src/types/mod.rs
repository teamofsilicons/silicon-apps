//! Request and response types mirroring the Silicon Accounts HTTP API.
//!
//! Response types tolerate fields the service adds later and `null` where a value is
//! usually present. Request types implement `Default` so you can write
//! `CreateSilicon { id: "si:scout".into(), display_name: "Scout".into(), ..Default::default() }`.

mod account;
mod apps;
mod meta;
mod proofs;
mod silicons;
mod tokens;

pub use account::*;
pub use apps::*;
pub use meta::*;
pub use proofs::*;
pub use silicons::*;
pub use tokens::*;

use serde::{Deserialize, Serialize};

/// One page of a list endpoint: `{"items":[…],"next_cursor":null}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    /// The items on this page.
    pub items: Vec<T>,
    /// Pass this as `cursor` to get the next page; `None` on the last page.
    #[serde(default)]
    pub next_cursor: Option<String>,
}

impl<T> Default for Page<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            next_cursor: None,
        }
    }
}

impl<T> Page<T> {
    /// True when there are no more pages.
    pub fn is_last(&self) -> bool {
        self.next_cursor.is_none()
    }
}

impl<T> IntoIterator for Page<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

/// Paging parameters for list endpoints (`limit` max 200).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageRequest {
    /// Items per page (1..=200, default 50 on the service).
    pub limit: Option<u32>,
    /// The `next_cursor` of the previous page.
    pub cursor: Option<String>,
}
