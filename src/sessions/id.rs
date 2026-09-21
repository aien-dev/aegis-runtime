use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident, $prefix:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, Uuid::new_v4().simple()))
            }

            pub fn from_string(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

define_id!(SessionId, "sess");
define_id!(RunId, "run");
define_id!(ActionId, "act");
define_id!(ApprovalId, "appr");
define_id!(MessageId, "msg");

impl From<SessionId> for aien_protocol_types::SessionId {
    fn from(s: SessionId) -> Self {
        let raw = s.as_str().strip_prefix("sess_").unwrap_or(s.as_str());
        if let Ok(u) = uuid::Uuid::parse_str(raw) {
            aien_protocol_types::SessionId(u)
        } else {
            aien_protocol_types::SessionId(uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_OID,
                s.as_str().as_bytes(),
            ))
        }
    }
}

impl From<aien_protocol_types::SessionId> for SessionId {
    fn from(id: aien_protocol_types::SessionId) -> Self {
        Self::from_string(format!("sess_{}", id.0.simple()))
    }
}

impl From<RunId> for aien_protocol_types::RunId {
    fn from(r: RunId) -> Self {
        let raw = r.as_str().strip_prefix("run_").unwrap_or(r.as_str());
        if let Ok(u) = uuid::Uuid::parse_str(raw) {
            aien_protocol_types::RunId(u)
        } else {
            aien_protocol_types::RunId(uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_OID,
                r.as_str().as_bytes(),
            ))
        }
    }
}

impl From<aien_protocol_types::RunId> for RunId {
    fn from(id: aien_protocol_types::RunId) -> Self {
        Self::from_string(format!("run_{}", id.0.simple()))
    }
}

impl From<ActionId> for aien_protocol_types::ActionId {
    fn from(a: ActionId) -> Self {
        let raw = a.as_str().strip_prefix("act_").unwrap_or(a.as_str());
        if let Ok(u) = uuid::Uuid::parse_str(raw) {
            aien_protocol_types::ActionId(u)
        } else {
            aien_protocol_types::ActionId(uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_OID,
                a.as_str().as_bytes(),
            ))
        }
    }
}

impl From<aien_protocol_types::ActionId> for ActionId {
    fn from(id: aien_protocol_types::ActionId) -> Self {
        Self::from_string(format!("act_{}", id.0.simple()))
    }
}

impl From<ApprovalId> for aien_protocol_types::ApprovalId {
    fn from(a: ApprovalId) -> Self {
        let raw = a.as_str().strip_prefix("appr_").unwrap_or(a.as_str());
        if let Ok(u) = uuid::Uuid::parse_str(raw) {
            aien_protocol_types::ApprovalId(u)
        } else {
            aien_protocol_types::ApprovalId(uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_OID,
                a.as_str().as_bytes(),
            ))
        }
    }
}

impl From<aien_protocol_types::ApprovalId> for ApprovalId {
    fn from(id: aien_protocol_types::ApprovalId) -> Self {
        Self::from_string(format!("appr_{}", id.0.simple()))
    }
}
