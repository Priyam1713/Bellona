//! Identifiers. Newtypes so the type system separates realms.

use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fmt;

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Opaque identifier with a readable prefix, e.g. `act_9f2c41ab`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Id(pub String);

impl Id {
    /// Mint a fresh id with the given prefix and 8 random hex chars.
    pub fn mint(prefix: &str) -> Self {
        let mut bytes = [0u8; 4];
        rand::thread_rng().fill_bytes(&mut bytes);
        let mut s = String::with_capacity(prefix.len() + 9);
        s.push_str(prefix);
        s.push('_');
        for b in bytes {
            s.push(HEX[(b >> 4) as usize] as char);
            s.push(HEX[(b & 0xf) as usize] as char);
        }
        Id(s)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

macro_rules! realm_id {
    ($name:ident, $prefix:literal, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub Id);

        impl $name {
            /// Mint a fresh identifier in this realm.
            pub fn mint() -> Self {
                $name(Id::mint($prefix))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

realm_id!(AgentId, "agt", "An actor in the war camp.");
realm_id!(SessionId, "ses", "One durable campaign thread.");
realm_id!(ActionId, "act", "One proposed effect awaiting judgment.");
realm_id!(RunId, "run", "One execution of the war loop.");
