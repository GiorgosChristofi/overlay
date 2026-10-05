use std::fmt;
use thiserror::Error;

/// Connection details for a running League client.
#[derive(Clone, PartialEq, Eq)]
pub struct LockfileInfo {
    pub pid: u32,
    pub port: u16,
    pub password: String,
    /// `https` for the real client; tests use `http`.
    pub protocol: String,
}

// Never print the auth token.
impl fmt::Debug for LockfileInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LockfileInfo")
            .field("pid", &self.pid)
            .field("port", &self.port)
            .field("password", &"<redacted>")
            .field("protocol", &self.protocol)
            .finish()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LockfileError {
    #[error("lockfile has {0} fields, expected 5")]
    FieldCount(usize),
    #[error("lockfile field `{0}` is not a number")]
    NotANumber(&'static str),
}

impl LockfileInfo {
    /// Parses `name:pid:port:password:protocol`.
    pub fn parse(raw: &str) -> Result<Self, LockfileError> {
        let parts: Vec<&str> = raw.trim().split(':').collect();
        let [_name, pid, port, password, protocol] = parts.as_slice() else {
            return Err(LockfileError::FieldCount(parts.len()));
        };
        Ok(Self {
            pid: pid.parse().map_err(|_| LockfileError::NotANumber("pid"))?,
            port: port
                .parse()
                .map_err(|_| LockfileError::NotANumber("port"))?,
            password: (*password).to_owned(),
            protocol: (*protocol).to_owned(),
        })
    }

    pub fn base_url(&self) -> String {
        format!("{}://127.0.0.1:{}", self.protocol, self.port)
    }

    pub fn ws_url(&self) -> String {
        let scheme = if self.protocol == "https" {
            "wss"
        } else {
            "ws"
        };
        format!("{scheme}://127.0.0.1:{}/", self.port)
    }

    pub fn is_tls(&self) -> bool {
        self.protocol == "https"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_format() {
        let info = LockfileInfo::parse("LeagueClient:20584:61437:Xy-z_9:https\n").unwrap();
        assert_eq!(info.pid, 20584);
        assert_eq!(info.port, 61437);
        assert_eq!(info.password, "Xy-z_9");
        assert_eq!(info.base_url(), "https://127.0.0.1:61437");
        assert_eq!(info.ws_url(), "wss://127.0.0.1:61437/");
    }

    #[test]
    fn rejects_truncated_file() {
        assert_eq!(
            LockfileInfo::parse("LeagueClient:1:2"),
            Err(LockfileError::FieldCount(3))
        );
    }

    #[test]
    fn debug_hides_password() {
        let info = LockfileInfo::parse("LeagueClient:1:2:secret:https").unwrap();
        assert!(!format!("{info:?}").contains("secret"));
    }
}
