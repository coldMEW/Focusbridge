use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};
use url::Url;

pub const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

pub fn code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn google_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    verifier: &str,
) -> String {
    let mut url = Url::parse(GOOGLE_AUTH_URL).expect("constant google auth url is valid");
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", "openid email profile")
        .append_pair("state", state)
        .append_pair("code_challenge", &code_challenge(verifier))
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "select_account");
    url.to_string()
}

pub fn callback_code_and_state(request: &str) -> Option<(String, String)> {
    let first_line = request.lines().next()?;
    let path = first_line.split_whitespace().nth(1)?;
    let url = Url::parse(&format!("http://127.0.0.1{path}")).ok()?;
    let mut code = None;
    let mut state = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.to_string()),
            "state" => state = Some(value.to_string()),
            _ => {}
        }
    }
    Some((code?, state?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_matches_pkce_s256_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

        assert_eq!(
            code_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn authorize_url_contains_pkce_and_openid_scope() {
        let url = google_authorize_url(
            "client.apps.googleusercontent.com",
            "http://127.0.0.1:45555/oauth/google/callback",
            "state-123",
            "verifier-123",
        );

        assert!(url.starts_with(GOOGLE_AUTH_URL));
        assert!(url.contains("client_id=client.apps.googleusercontent.com"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=openid+email+profile"));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn callback_parser_extracts_code_and_state() {
        let request =
            "GET /oauth/google/callback?state=state-123&code=code-456 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";

        assert_eq!(
            callback_code_and_state(request),
            Some(("code-456".into(), "state-123".into()))
        );
    }
}
