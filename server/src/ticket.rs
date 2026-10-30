//! Game ticket verification. Tickets are short-lived EdDSA JWTs signed by rots-authserver; the
//! game server checks them offline against the authserver's public key.

use bevy::prelude::*;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, errors::ErrorKind};
use serde::Deserialize;
use shared::event::PlayerId;

const TICKET_ISSUER: &str = "rots-authserver";

#[derive(Resource, Clone)]
pub enum TicketVerifier {
    /// Accept every connection with a random `PlayerId`. Used by singleplayer and
    /// `ROTS_AUTH_DISABLED=1`.
    Disabled,
    Ed25519 {
        key: DecodingKey,
        audience: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedTicket {
    pub player_id: PlayerId,
    /// Character name, only trusted for account tickets. Guests keep the name they asked for.
    pub name: Option<String>,
}

/// The claims the game server relies on. `iss`, `aud` and `exp` are checked by `Validation`; the
/// remaining authserver claims (`sid`, `prov`, `acc`, `iat`) are ignored here.
#[derive(Deserialize)]
struct GameTicketClaims {
    sub: String,
    kind: String,
    name: String,
}

impl TicketVerifier {
    pub fn verify(&self, ticket: &str) -> Result<VerifiedTicket, String> {
        let (key, audience) = match self {
            TicketVerifier::Disabled => {
                return Ok(VerifiedTicket {
                    player_id: PlayerId::random(),
                    name: None,
                });
            }
            TicketVerifier::Ed25519 { key, audience } => (key, audience),
        };

        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.set_audience(&[audience]);
        validation.set_issuer(&[TICKET_ISSUER]);

        let claims = jsonwebtoken::decode::<GameTicketClaims>(ticket, key, &validation)
            .map_err(|e| match e.kind() {
                ErrorKind::ExpiredSignature => "ticket expired".to_string(),
                _ => "invalid ticket".to_string(),
            })?
            .claims;

        let player_id = PlayerId(
            claims
                .sub
                .parse::<u64>()
                .map_err(|_| "invalid ticket".to_string())?,
        );

        Ok(VerifiedTicket {
            player_id,
            name: (claims.kind == "account").then_some(claims.name),
        })
    }

    /// Build the verifier from the environment:
    /// - `ROTS_AUTH_DISABLED=1` disables verification.
    /// - Otherwise `ROTS_AUTH_URL` and `ROTS_TICKET_AUDIENCE` are required, and the public key is
    ///   fetched from `{ROTS_AUTH_URL}/api/v1/ticket/public-key`, retrying while the authserver
    ///   starts.
    ///
    /// Runs before bevy's `LogPlugin` exists, so progress goes to stderr.
    pub async fn from_env() -> Self {
        if std::env::var("ROTS_AUTH_DISABLED").as_deref() == Ok("1") {
            return TicketVerifier::Disabled;
        }

        let auth_url = non_empty_env("ROTS_AUTH_URL")
            .expect("ROTS_AUTH_URL or ROTS_AUTH_DISABLED=1 required");
        let audience = non_empty_env("ROTS_TICKET_AUDIENCE").expect("ROTS_TICKET_AUDIENCE required");

        let x = fetch_public_key(auth_url.trim_end_matches('/')).await;
        let key = DecodingKey::from_ed_components(&x)
            .expect("authserver public key is not valid base64url");
        // The EdDSA verifier slices the first 32 bytes; reject anything shorter up front instead
        // of panicking on the first connect.
        assert_eq!(
            key.try_get_as_bytes().map(<[u8]>::len).ok(),
            Some(32),
            "authserver public key must be 32 bytes"
        );

        eprintln!("ticket verification enabled, audience {audience}");
        TicketVerifier::Ed25519 { key, audience }
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

#[derive(Deserialize)]
struct PublicKeyResponse {
    alg: String,
    x: String,
}

async fn fetch_public_key(auth_url: &str) -> String {
    const ATTEMPTS: u32 = 60;
    let url = format!("{auth_url}/api/v1/ticket/public-key");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("failed to build HTTP client");

    for attempt in 1..=ATTEMPTS {
        let result = async {
            client
                .get(&url)
                .send()
                .await?
                .error_for_status()?
                .json::<PublicKeyResponse>()
                .await
        }
        .await;

        match result {
            Ok(body) if body.alg == "EdDSA" => return body.x,
            Ok(body) => panic!("authserver public key has unsupported alg {:?}", body.alg),
            Err(e) => {
                eprintln!("fetching {url} failed (attempt {attempt}/{ATTEMPTS}): {e}");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
    }
    panic!("could not fetch ticket public key from {url} after {ATTEMPTS} attempts");
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use ed25519_dalek::{SigningKey, pkcs8::EncodePrivateKey};
    use jsonwebtoken::{EncodingKey, Header};
    use serde::Serialize;

    const AUDIENCE: &str = "https://rots.test";

    /// Mirrors rots-authserver's `GameTicketClaims` (plan A4).
    #[derive(Serialize)]
    struct AuthserverClaims {
        iss: String,
        aud: String,
        sub: String,
        sid: String,
        kind: String,
        prov: Option<String>,
        acc: Option<i64>,
        name: String,
        iat: u64,
        exp: u64,
    }

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn claims(kind: &str, aud: &str, iat: u64) -> AuthserverClaims {
        let account = kind == "account";
        AuthserverClaims {
            iss: TICKET_ISSUER.to_string(),
            aud: aud.to_string(),
            sub: "42".to_string(),
            sid: "0123456789abcdef".to_string(),
            kind: kind.to_string(),
            prov: account.then(|| "steam".to_string()),
            acc: account.then_some(7),
            name: if account { "Steam 765" } else { "Guest-42" }.to_string(),
            iat,
            exp: iat + 300,
        }
    }

    /// A fresh signing key plus the verifier the game server would build from the authserver's
    /// `{"x": ...}` public key response.
    fn keypair() -> (EncodingKey, TicketVerifier) {
        let signing_key = SigningKey::from_bytes(&rand::random::<[u8; 32]>());
        let der = signing_key.to_pkcs8_der().unwrap();
        let encoding = EncodingKey::from_ed_der(der.as_bytes());
        let x = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(signing_key.verifying_key().to_bytes());
        let verifier = TicketVerifier::Ed25519 {
            key: DecodingKey::from_ed_components(&x).unwrap(),
            audience: AUDIENCE.to_string(),
        };
        (encoding, verifier)
    }

    fn sign(key: &EncodingKey, claims: &AuthserverClaims) -> String {
        jsonwebtoken::encode(&Header::new(Algorithm::EdDSA), claims, key).unwrap()
    }

    #[test]
    fn valid_guest_ticket_yields_sub_as_player_id_without_name() {
        let (key, verifier) = keypair();
        let ticket = sign(&key, &claims("guest", AUDIENCE, now()));
        assert_eq!(
            verifier.verify(&ticket),
            Ok(VerifiedTicket {
                player_id: PlayerId(42),
                name: None,
            })
        );
    }

    #[test]
    fn valid_account_ticket_carries_character_name() {
        let (key, verifier) = keypair();
        let ticket = sign(&key, &claims("account", AUDIENCE, now()));
        assert_eq!(
            verifier.verify(&ticket),
            Ok(VerifiedTicket {
                player_id: PlayerId(42),
                name: Some("Steam 765".to_string()),
            })
        );
    }

    #[test]
    fn wrong_audience_is_invalid() {
        let (key, verifier) = keypair();
        let ticket = sign(&key, &claims("guest", "http://other", now()));
        assert_eq!(verifier.verify(&ticket), Err("invalid ticket".to_string()));
    }

    #[test]
    fn wrong_issuer_is_invalid() {
        let (key, verifier) = keypair();
        let mut c = claims("guest", AUDIENCE, now());
        c.iss = "someone-else".to_string();
        assert_eq!(
            verifier.verify(&sign(&key, &c)),
            Err("invalid ticket".to_string())
        );
    }

    #[test]
    fn ticket_signed_by_another_key_is_invalid() {
        let (other_key, _) = keypair();
        let (_, verifier) = keypair();
        let ticket = sign(&other_key, &claims("guest", AUDIENCE, now()));
        assert_eq!(verifier.verify(&ticket), Err("invalid ticket".to_string()));
    }

    #[test]
    fn past_exp_is_expired() {
        let (key, verifier) = keypair();
        // exp = now - 600, well outside jsonwebtoken's default 60s leeway.
        let ticket = sign(&key, &claims("guest", AUDIENCE, now() - 900));
        assert_eq!(verifier.verify(&ticket), Err("ticket expired".to_string()));
    }

    #[test]
    fn non_numeric_sub_is_invalid() {
        let (key, verifier) = keypair();
        let mut c = claims("guest", AUDIENCE, now());
        c.sub = "not-a-number".to_string();
        assert_eq!(
            verifier.verify(&sign(&key, &c)),
            Err("invalid ticket".to_string())
        );
    }

    #[test]
    fn garbage_and_empty_tickets_are_invalid() {
        let (_, verifier) = keypair();
        assert_eq!(verifier.verify(""), Err("invalid ticket".to_string()));
        assert_eq!(
            verifier.verify("a.b.c"),
            Err("invalid ticket".to_string())
        );
    }

    #[test]
    fn disabled_accepts_anything_with_random_ids() {
        let a = TicketVerifier::Disabled.verify("").unwrap();
        let b = TicketVerifier::Disabled.verify("garbage").unwrap();
        assert_eq!(a.name, None);
        assert_ne!(a.player_id, b.player_id);
    }
}
