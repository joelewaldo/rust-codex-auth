use std::{fmt, fs, path::Path};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthInfo {
    pub email: String,
    pub user_id: String,
    pub account_id: String,
    pub access_token: String,
    pub plan: Option<String>,
}

impl AuthInfo {
    pub fn record_key(&self) -> String {
        format!("{}::{}", self.user_id, self.account_id)
    }
}

#[derive(Debug)]
pub enum AuthError {
    Io(std::io::Error),
    Json(serde_json::Error),
    ApiKeyUnsupported,
    MissingField(&'static str),
    InvalidJwt,
    AccountIdMismatch,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Json(err) => write!(f, "{err}"),
            Self::ApiKeyUnsupported => {
                write!(f, "API-key auth is not supported; use ChatGPT OAuth login")
            }
            Self::MissingField(field) => write!(f, "auth.json is missing `{field}`"),
            Self::InvalidJwt => write!(f, "auth.json contains an invalid JWT"),
            Self::AccountIdMismatch => {
                write!(
                    f,
                    "auth.json token account id does not match JWT account id"
                )
            }
        }
    }
}

impl std::error::Error for AuthError {}

impl From<std::io::Error> for AuthError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for AuthError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub fn parse_file(path: &Path) -> Result<AuthInfo, AuthError> {
    let bytes = fs::read(path)?;
    parse_bytes(&bytes)
}

pub fn parse_bytes(bytes: &[u8]) -> Result<AuthInfo, AuthError> {
    let root: Value = serde_json::from_slice(bytes)?;
    if root
        .get("OPENAI_API_KEY")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(AuthError::ApiKeyUnsupported);
    }

    let tokens = root
        .get("tokens")
        .and_then(Value::as_object)
        .ok_or(AuthError::MissingField("tokens"))?;
    let access_token = required_string(tokens.get("access_token"), "tokens.access_token")?;
    let id_token = required_string(tokens.get("id_token"), "tokens.id_token")?;
    let token_account_id = optional_non_empty_string(tokens.get("account_id"));

    let claims = decode_jwt_payload(id_token)?;
    let email = required_string(claims.get("email"), "email")?.to_ascii_lowercase();
    let auth_claims = claims
        .get("https://api.openai.com/auth")
        .and_then(Value::as_object)
        .ok_or(AuthError::MissingField("https://api.openai.com/auth"))?;

    let jwt_account_id = optional_non_empty_string(auth_claims.get("chatgpt_account_id"))
        .or_else(|| default_organization_id(auth_claims.get("organizations")));
    let account_id = match (token_account_id, jwt_account_id) {
        (Some(token), Some(jwt)) if token != jwt => return Err(AuthError::AccountIdMismatch),
        (Some(token), _) => token.to_owned(),
        (_, Some(jwt)) => jwt.to_owned(),
        (None, None) => return Err(AuthError::MissingField("chatgpt_account_id")),
    };
    let user_id = optional_non_empty_string(auth_claims.get("chatgpt_user_id"))
        .or_else(|| optional_non_empty_string(auth_claims.get("user_id")))
        .ok_or(AuthError::MissingField("chatgpt_user_id"))?
        .to_owned();
    let plan = optional_non_empty_string(auth_claims.get("chatgpt_plan_type")).map(str::to_owned);

    Ok(AuthInfo {
        email,
        user_id,
        account_id,
        access_token: access_token.to_owned(),
        plan,
    })
}

fn required_string<'a>(
    value: Option<&'a Value>,
    field: &'static str,
) -> Result<&'a str, AuthError> {
    optional_non_empty_string(value).ok_or(AuthError::MissingField(field))
}

fn optional_non_empty_string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn decode_jwt_payload(jwt: &str) -> Result<Value, AuthError> {
    let mut parts = jwt.split('.');
    let _header = parts.next().ok_or(AuthError::InvalidJwt)?;
    let payload = parts.next().ok_or(AuthError::InvalidJwt)?;
    let _signature = parts.next().ok_or(AuthError::InvalidJwt)?;
    if parts.next().is_some() {
        return Err(AuthError::InvalidJwt);
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| AuthError::InvalidJwt)?;
    serde_json::from_slice(&decoded).map_err(|_| AuthError::InvalidJwt)
}

fn default_organization_id(value: Option<&Value>) -> Option<&str> {
    let organizations = value?.as_array()?;
    organizations
        .iter()
        .find(|org| org.get("is_default").and_then(Value::as_bool) == Some(true))
        .and_then(|org| optional_non_empty_string(org.get("id")))
        .or_else(|| {
            organizations
                .iter()
                .find_map(|org| optional_non_empty_string(org.get("id")))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use serde_json::json;

    fn jwt(payload: Value) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
        format!("{header}.{payload}.sig")
    }

    fn auth_json(tokens_account_id: &str, payload: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "tokens": {
                "access_token": "access-token",
                "account_id": tokens_account_id,
                "id_token": jwt(payload)
            }
        }))
        .unwrap()
    }

    #[test]
    fn parses_standard_chatgpt_auth() {
        let bytes = auth_json(
            "acct-1",
            json!({
                "email": "User@Example.com",
                "https://api.openai.com/auth": {
                    "chatgpt_account_id": "acct-1",
                    "chatgpt_user_id": "user-1",
                    "chatgpt_plan_type": "pro"
                }
            }),
        );

        let info = parse_bytes(&bytes).unwrap();

        assert_eq!(info.email, "user@example.com");
        assert_eq!(info.user_id, "user-1");
        assert_eq!(info.account_id, "acct-1");
        assert_eq!(info.plan.as_deref(), Some("pro"));
        assert_eq!(info.record_key(), "user-1::acct-1");
    }

    #[test]
    fn uses_default_organization_when_account_id_is_missing() {
        let bytes = auth_json(
            "",
            json!({
                "email": "org@example.com",
                "https://api.openai.com/auth": {
                    "user_id": "user-org",
                    "organizations": [
                        {"id": "org-secondary", "is_default": false},
                        {"id": "org-primary", "is_default": true}
                    ]
                }
            }),
        );

        let info = parse_bytes(&bytes).unwrap();

        assert_eq!(info.account_id, "org-primary");
        assert_eq!(info.user_id, "user-org");
    }

    #[test]
    fn rejects_mismatched_account_ids() {
        let bytes = auth_json(
            "acct-token",
            json!({
                "email": "user@example.com",
                "https://api.openai.com/auth": {
                    "chatgpt_account_id": "acct-jwt",
                    "chatgpt_user_id": "user-1"
                }
            }),
        );

        assert!(matches!(
            parse_bytes(&bytes),
            Err(AuthError::AccountIdMismatch)
        ));
    }

    #[test]
    fn rejects_api_key_auth() {
        let bytes = serde_json::to_vec(&json!({
            "OPENAI_API_KEY": "sk-test"
        }))
        .unwrap();

        assert!(matches!(
            parse_bytes(&bytes),
            Err(AuthError::ApiKeyUnsupported)
        ));
    }
}
