//! Game ticket verification. Tickets are short-lived EdDSA JWTs signed by rots-authserver; the
//! game server checks them offline against the authserver's public key.

use bevy::prelude::*;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, errors::ErrorKind};
use serde::Deserialize;
use shared::event::PlayerId;

const TICKET_ISSUER: &str = "rots-authserver";

#[derive(Resource, Clone)]
pub enum TicketVerifier {
    /// Accept every connection with a random `PlayerId`. Used by the local test server and
    /// `ROTS_AUTH_DISABLED=1`.
    Disabled,
    Ed25519 {
        key: DecodingKey,
        audience: String,
        /// This server's slot (`ROTS_SERVER_SLOT`). Tickets carry the slot the coordinator
        /// assigned (`srv`); a ticket for another slot is refused.
        slot: Option<u32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedTicket {
    pub player_id: PlayerId,
    /// Character name, only trusted for account tickets. Guests keep the name they asked for.
    pub name: Option<String>,
    /// The login session id (`sid`), used to check at connect time that the login still exists.
    /// `None` when verification is disabled.
    pub sid: Option<String>,
}

/// The claims the game server relies on. `iss`, `aud` and `exp` are checked by `Validation`; the
/// remaining authserver claims (`prov`, `acc`, `iat`) are ignored here.
#[derive(Deserialize)]
struct GameTicketClaims {
    sub: String,
    sid: String,
    kind: String,
    name: String,
    /// Slot the authserver assigned. Absent in tickets minted before the coordinator existed.
    #[serde(default)]
    srv: Option<u32>,
}

impl TicketVerifier {
    pub fn verify(&self, ticket: &str) -> Result<VerifiedTicket, String> {
        let (key, audience, slot) = match self {
            TicketVerifier::Disabled => {
                return Ok(VerifiedTicket {
                    player_id: PlayerId::random(),
                    name: None,
                    sid: None,
                });
            }
            TicketVerifier::Ed25519 {
                key,
                audience,
                slot,
            } => (key, audience, slot),
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

        // The coordinator assigns a slot; a ticket presented to a different server is refused even
        // though the signature is valid.
        if let Some(slot) = slot {
            if claims.srv != Some(*slot) {
                return Err("ticket is for another server".to_string());
            }
        }

        let player_id = PlayerId(
            claims
                .sub
                .parse::<u64>()
                .map_err(|_| "invalid ticket".to_string())?,
        );

        Ok(VerifiedTicket {
            player_id,
            name: (claims.kind == "account").then_some(claims.name),
            sid: Some(claims.sid),
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

        let auth_url =
            non_empty_env("ROTS_AUTH_URL").expect("ROTS_AUTH_URL or ROTS_AUTH_DISABLED=1 required");
        let audience =
            non_empty_env("ROTS_TICKET_AUDIENCE").expect("ROTS_TICKET_AUDIENCE required");

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
        let slot = non_empty_env("ROTS_SERVER_SLOT").map(|v| {
            v.parse::<u32>()
                .expect("ROTS_SERVER_SLOT must be a non-negative integer")
        });
        if let Some(slot) = slot {
            eprintln!("ticket verification: this server is slot {slot}");
        }
        TicketVerifier::Ed25519 {
            key,
            audience,
            slot,
        }
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
        srv: Option<u32>,
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
            srv: None,
            exp: iat + 300,
        }
    }

    /// A fresh signing key plus the verifier the game server would build from the authserver's
    /// `{"x": ...}` public key response.
    fn keypair() -> (EncodingKey, TicketVerifier) {
        keypair_with_slot(None)
    }

    /// The same, for a server that knows its coordinator-assigned slot.
    fn keypair_with_slot(slot: Option<u32>) -> (EncodingKey, TicketVerifier) {
        let signing_key = SigningKey::from_bytes(&rand::random::<[u8; 32]>());
        let der = signing_key.to_pkcs8_der().unwrap();
        let encoding = EncodingKey::from_ed_der(der.as_bytes());
        let x = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(signing_key.verifying_key().to_bytes());
        let verifier = TicketVerifier::Ed25519 {
            key: DecodingKey::from_ed_components(&x).unwrap(),
            audience: AUDIENCE.to_string(),
            slot,
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
                sid: Some("0123456789abcdef".to_string()),
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
                sid: Some("0123456789abcdef".to_string()),
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
        assert_eq!(verifier.verify("a.b.c"), Err("invalid ticket".to_string()));
    }

    fn claims_with_srv(srv: Option<u32>) -> AuthserverClaims {
        AuthserverClaims {
            srv,
            ..claims("guest", AUDIENCE, now())
        }
    }

    #[test]
    fn ticket_for_this_slot_is_accepted() {
        let (key, verifier) = keypair_with_slot(Some(1));
        let ticket = sign(&key, &claims_with_srv(Some(1)));
        assert_eq!(verifier.verify(&ticket).unwrap().player_id, PlayerId(42));
    }

    #[test]
    fn ticket_for_another_slot_is_rejected() {
        let (key, verifier) = keypair_with_slot(Some(1));
        let ticket = sign(&key, &claims_with_srv(Some(2)));
        assert_eq!(
            verifier.verify(&ticket),
            Err("ticket is for another server".to_string())
        );
    }

    #[test]
    fn ticket_without_srv_is_rejected_when_the_server_has_a_slot() {
        let (key, verifier) = keypair_with_slot(Some(0));
        let ticket = sign(&key, &claims_with_srv(None));
        assert_eq!(
            verifier.verify(&ticket),
            Err("ticket is for another server".to_string())
        );
    }

    #[test]
    fn server_without_a_slot_accepts_any_srv() {
        let (key, verifier) = keypair();
        for srv in [None, Some(0), Some(7)] {
            let ticket = sign(&key, &claims_with_srv(srv));
            assert!(verifier.verify(&ticket).is_ok(), "srv {srv:?}");
        }
    }

    #[test]
    fn disabled_accepts_anything_with_random_ids() {
        let a = TicketVerifier::Disabled.verify("").unwrap();
        let b = TicketVerifier::Disabled.verify("garbage").unwrap();
        assert_eq!(a.name, None);
        assert_eq!(a.sid, None);
        assert_ne!(a.player_id, b.player_id);
    }
}
