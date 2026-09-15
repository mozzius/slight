use gateway_protocol::{ClientHello, ProtocolError};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    ReadSessions,
    SendInput,
    RespondPermission,
    CreateSession,
    ManageHost,
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub device_id: String,
    pub device_label: String,
    pub scopes: BTreeSet<Scope>,
}

impl AuthContext {
    pub fn all(device_id: impl Into<String>, device_label: impl Into<String>) -> Self {
        let mut scopes = BTreeSet::new();
        scopes.insert(Scope::ReadSessions);
        scopes.insert(Scope::SendInput);
        scopes.insert(Scope::RespondPermission);
        scopes.insert(Scope::CreateSession);
        scopes.insert(Scope::ManageHost);
        Self {
            device_id: device_id.into(),
            device_label: device_label.into(),
            scopes,
        }
    }

    pub fn has(&self, scope: Scope) -> bool {
        self.scopes.contains(&scope)
    }
}

pub trait Authenticator: Send + Sync {
    fn authenticate(&self, hello: &ClientHello) -> Result<AuthContext, ProtocolError>;
}
