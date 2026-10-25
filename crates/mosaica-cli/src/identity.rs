//! The identity verbs: logging in and out on the viewer plane, minting and revoking sessions on
//! the session plane, and the catalogue's verbs on the control plane. Each is one request; the
//! server's JSON answer is printed on stdout, and a refusal on stderr with exit 1.
//!
//! Secrets are never arguments, which other users of the machine can read: a password, an API key
//! or an access token to log in with is read from stdin, the session token to log out with from
//! `MOSAICA_TOKEN`, the session plane's credential from `MOSAICA_API_KEY`, and the control plane's
//! credential from `MOSAICA_CREDENTIAL`. Where `MOSAICA_API_KEY` or `MOSAICA_CREDENTIAL` is unset,
//! the request is sent without a credential, as an open server serves it.

use std::process::ExitCode;

use clap::{Args, Subcommand};
use serde_json::{json, Value};

use crate::http::{env_bearer, env_secret, finish, segment, send_json, stdin_secret, Control, Target};

#[derive(Args)]
#[group(required = true, multiple = false)]
pub(crate) struct LoginCredential {
    /// Log in as this local principal, with the password read from the first line of stdin.
    #[arg(long, value_name = "NAME")]
    principal: Option<String>,
    /// Log in with the API key read from the first line of stdin.
    #[arg(long)]
    api_key: bool,
    /// Log in with the OIDC access token read from the first line of stdin.
    #[arg(long)]
    access_token: bool,
}

#[derive(Args)]
pub(crate) struct LoginArgs {
    /// The viewer plane's address, such as `http://127.0.0.1:8080`.
    #[arg(long, value_name = "URL")]
    server: String,
    #[command(flatten)]
    credential: LoginCredential,
}

pub(crate) fn login(args: LoginArgs) -> ExitCode {
    let answer = (|| {
        let target = Target::parse(&args.server)?;
        let c = &args.credential;
        let body = match (&c.principal, c.api_key, c.access_token) {
            (Some(principal), _, _) => json!({ "password": {
                "principal": principal,
                "password": stdin_secret("password")?,
            } }),
            (None, true, _) => json!({ "api_key": stdin_secret("API key")? }),
            _ => json!({ "access_token": stdin_secret("access token")? }),
        };
        send_json(&target, "POST", "/v1/login", None, Some(&body))
    })();
    finish("login", answer)
}

#[derive(Args)]
pub(crate) struct LogoutArgs {
    /// The viewer plane's address, such as `http://127.0.0.1:8080`. The session token to end is
    /// read from `MOSAICA_TOKEN`.
    #[arg(long, value_name = "URL")]
    server: String,
}

pub(crate) fn logout(args: LogoutArgs) -> ExitCode {
    let answer = (|| {
        let target = Target::parse(&args.server)?;
        let token = env_secret("MOSAICA_TOKEN", "session token")?;
        send_json(&target, "POST", "/v1/logout", Some(&token), None)
    })();
    finish("logout", answer)
}

#[derive(Subcommand)]
pub(crate) enum SessionCommand {
    /// Mint a session on the session plane, with the credential in `MOSAICA_API_KEY`: an API key
    /// whose principal holds `authorise-as`, or the operator credential.
    ///
    /// A session for a principal carries the target's terms and its `read` and `write`, and never
    /// its `read-all` or `write-all`. With the operator credential alone, a session for `--term`s
    /// holds those terms and `read`, and a session for `--read-all` reads every item. It prints
    /// `token`, `token_id` and `expires_at` as JSON.
    Authorise {
        /// The session plane's address, such as `http://127.0.0.1:8081`.
        #[arg(long, value_name = "URL")]
        session: String,
        #[command(flatten)]
        target: AuthoriseTarget,
    },
    /// End a session by its `token_id` on the session plane, with the credential in
    /// `MOSAICA_API_KEY`: an API key ends a session minted with a key of the same principal, and
    /// the operator credential ends any session.
    Revoke {
        /// The session plane's address, such as `http://127.0.0.1:8081`.
        #[arg(long, value_name = "URL")]
        session: String,
        /// The `token_id` `mosaica session authorise` printed.
        #[arg(long, value_name = "N")]
        token_id: u64,
    },
    /// List live sessions on the control plane: all of them, or a local principal's or a
    /// provider's.
    List {
        #[command(flatten)]
        control: Control,
        /// Only this local principal's sessions, including those minted for it.
        #[arg(long, value_name = "NAME", conflicts_with = "provider")]
        principal: Option<String>,
        /// Only the sessions authorised through this provider.
        #[arg(long, value_name = "NAME")]
        provider: Option<String>,
    },
    /// End one session by `token_id`, or every session of a local principal or a provider, on the
    /// control plane.
    End {
        #[command(flatten)]
        control: Control,
        #[command(flatten)]
        which: EndWhich,
    },
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub(crate) struct AuthoriseTarget {
    /// Act as this local principal.
    #[arg(long, value_name = "NAME")]
    principal: Option<String>,
    /// Act as the OIDC identity whose access token is read from the first line of stdin.
    #[arg(long)]
    access_token: bool,
    /// A term the session holds, with the operator credential. Repeatable.
    #[arg(long = "term", value_name = "TERM")]
    terms: Vec<String>,
    /// A session of the superuser itself, which reads every item, with the operator credential.
    #[arg(long)]
    read_all: bool,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub(crate) struct EndWhich {
    /// The one session with this `token_id`.
    #[arg(long, value_name = "N")]
    token_id: Option<u64>,
    /// Every session of this local principal, including those minted for it.
    #[arg(long, value_name = "NAME")]
    principal: Option<String>,
    /// Every session authorised through this provider.
    #[arg(long, value_name = "NAME")]
    provider: Option<String>,
}

pub(crate) fn session(command: SessionCommand) -> ExitCode {
    let answer = match command {
        SessionCommand::Authorise { session, target } => (|| {
            let at = Target::parse(&session)?;
            let key = env_bearer("MOSAICA_API_KEY");
            let body = match target {
                AuthoriseTarget {
                    principal: Some(principal),
                    ..
                } => json!({ "principal": principal }),
                AuthoriseTarget {
                    access_token: true, ..
                } => json!({ "access_token": stdin_secret("access token")? }),
                AuthoriseTarget { read_all: true, .. } => json!({ "read_all": true }),
                AuthoriseTarget { terms, .. } => json!({ "terms": terms }),
            };
            send_json(&at, "POST", "/session/authorise", key.as_deref(), Some(&body))
        })(),
        SessionCommand::Revoke { session, token_id } => (|| {
            let target = Target::parse(&session)?;
            let key = env_bearer("MOSAICA_API_KEY");
            let body = json!({ "token_id": token_id });
            send_json(&target, "POST", "/session/revoke", key.as_deref(), Some(&body))
        })(),
        SessionCommand::List {
            control,
            principal,
            provider,
        } => {
            let query = match (principal, provider) {
                (Some(p), _) => format!("?principal={}", segment(&p)),
                (None, Some(p)) => format!("?provider={}", segment(&p)),
                (None, None) => String::new(),
            };
            control.call("GET", &format!("/control/sessions{query}"), None)
        }
        SessionCommand::End { control, which } => {
            let body = match (which.token_id, which.principal, which.provider) {
                (Some(id), _, _) => json!({ "token_id": id }),
                (None, Some(p), _) => json!({ "principal": p }),
                (None, None, p) => json!({ "provider": p }),
            };
            control.call("POST", "/control/sessions/end", Some(body))
        }
    };
    finish("session", answer)
}

#[derive(Subcommand)]
pub(crate) enum PrincipalCommand {
    /// List every local principal.
    List {
        #[command(flatten)]
        control: Control,
    },
    /// Show one principal: its kind, flags, terms, permissions and groups.
    Show {
        #[command(flatten)]
        control: Control,
        /// The principal's name.
        name: String,
    },
    /// Create a local principal, holding nothing.
    Create {
        #[command(flatten)]
        control: Control,
        /// The principal's name.
        name: String,
        /// `person` or `service`.
        #[arg(long)]
        kind: String,
    },
    /// Delete a principal with its password, API keys, grants and memberships.
    Delete {
        #[command(flatten)]
        control: Control,
        /// The principal's name.
        name: String,
    },
    /// Disable a principal. Its sessions end, and none of its credentials is accepted.
    Disable {
        #[command(flatten)]
        control: Control,
        /// The principal's name.
        name: String,
    },
    /// Enable a disabled principal.
    Enable {
        #[command(flatten)]
        control: Control,
        /// The principal's name.
        name: String,
    },
    /// Set a principal's password, read from the first line of stdin.
    SetPassword {
        #[command(flatten)]
        control: Control,
        /// The principal's name.
        name: String,
    },
    /// Remove a principal's password.
    ClearPassword {
        #[command(flatten)]
        control: Control,
        /// The principal's name.
        name: String,
    },
}

pub(crate) fn principal(command: PrincipalCommand) -> ExitCode {
    let at = |name: &str| format!("/control/principals/{}", segment(name));
    let answer = match command {
        PrincipalCommand::List { control } => control.call("GET", "/control/principals", None),
        PrincipalCommand::Show { control, name } => control.call("GET", &at(&name), None),
        PrincipalCommand::Create {
            control,
            name,
            kind,
        } => control.call(
            "POST",
            "/control/principals",
            Some(json!({ "name": name, "kind": kind })),
        ),
        PrincipalCommand::Delete { control, name } => control.call("DELETE", &at(&name), None),
        PrincipalCommand::Disable { control, name } => {
            control.call("PATCH", &at(&name), Some(json!({ "disabled": true })))
        }
        PrincipalCommand::Enable { control, name } => {
            control.call("PATCH", &at(&name), Some(json!({ "disabled": false })))
        }
        PrincipalCommand::SetPassword { control, name } => stdin_secret("password").and_then(|p| {
            control.call(
                "PUT",
                &format!("{}/password", at(&name)),
                Some(json!({ "password": p })),
            )
        }),
        PrincipalCommand::ClearPassword { control, name } => {
            control.call("DELETE", &format!("{}/password", at(&name)), None)
        }
    };
    finish("principal", answer)
}

#[derive(Subcommand)]
pub(crate) enum KeyCommand {
    /// List a principal's API keys, by prefix, without their secrets.
    List {
        #[command(flatten)]
        control: Control,
        /// The principal's name.
        principal: String,
    },
    /// Issue an API key for a principal. The whole key is printed once, here.
    Create {
        #[command(flatten)]
        control: Control,
        /// The principal's name.
        principal: String,
        /// When the key, and every session authorised with it, ends, in seconds since the Unix
        /// epoch. Without it the key does not expire.
        #[arg(long, value_name = "SECONDS")]
        expires_at: Option<u64>,
        /// A permission the key holds: `read`, `write`, `authorise-as`, `admin`, `read-all` or
        /// `write-all`. Repeatable. Without it the key holds its principal's permissions.
        #[arg(long = "permission", value_name = "NAME")]
        permissions: Vec<String>,
    },
    /// Revoke an API key by its prefix. Every session authorised or minted with it ends.
    Revoke {
        #[command(flatten)]
        control: Control,
        /// The key's prefix, as `mosaica key list` prints it.
        prefix: String,
    },
}

pub(crate) fn key(command: KeyCommand) -> ExitCode {
    let answer = match command {
        KeyCommand::List { control, principal } => control.call(
            "GET",
            &format!("/control/principals/{}/keys", segment(&principal)),
            None,
        ),
        KeyCommand::Create {
            control,
            principal,
            expires_at,
            permissions,
        } => {
            let mut body = json!({});
            if let Some(at) = expires_at {
                body["expires_at"] = at.into();
            }
            if !permissions.is_empty() {
                body["permissions"] = permissions.into();
            }
            control.call(
                "POST",
                &format!("/control/principals/{}/keys", segment(&principal)),
                Some(body),
            )
        }
        KeyCommand::Revoke { control, prefix } => {
            control.call("DELETE", &format!("/control/keys/{}", segment(&prefix)), None)
        }
    };
    finish("key", answer)
}

#[derive(Subcommand)]
pub(crate) enum GroupCommand {
    /// List every local group.
    List {
        #[command(flatten)]
        control: Control,
    },
    /// Show one group: its terms, permissions and members.
    Show {
        #[command(flatten)]
        control: Control,
        /// The group's name.
        name: String,
    },
    /// Create a local group.
    Create {
        #[command(flatten)]
        control: Control,
        /// The group's name.
        name: String,
    },
    /// Delete a group with its grants and memberships.
    Delete {
        #[command(flatten)]
        control: Control,
        /// The group's name.
        name: String,
    },
    /// Add a principal to a group.
    AddMember {
        #[command(flatten)]
        control: Control,
        /// The group's name.
        group: String,
        /// The principal's name.
        principal: String,
    },
    /// Remove a principal from a group.
    RemoveMember {
        #[command(flatten)]
        control: Control,
        /// The group's name.
        group: String,
        /// The principal's name.
        principal: String,
    },
}

pub(crate) fn group(command: GroupCommand) -> ExitCode {
    let at = |name: &str| format!("/control/groups/{}", segment(name));
    let member = |g: &str, p: &str| format!("{}/members/{}", at(g), segment(p));
    let answer = match command {
        GroupCommand::List { control } => control.call("GET", "/control/groups", None),
        GroupCommand::Show { control, name } => control.call("GET", &at(&name), None),
        GroupCommand::Create { control, name } => {
            control.call("POST", "/control/groups", Some(json!({ "name": name })))
        }
        GroupCommand::Delete { control, name } => control.call("DELETE", &at(&name), None),
        GroupCommand::AddMember {
            control,
            group,
            principal,
        } => control.call("PUT", &member(&group, &principal), None),
        GroupCommand::RemoveMember {
            control,
            group,
            principal,
        } => control.call("DELETE", &member(&group, &principal), None),
    };
    finish("group", answer)
}

#[derive(Args)]
pub(crate) struct GrantArgs {
    #[command(flatten)]
    control: Control,
    #[command(flatten)]
    grantee: Grantee,
    #[command(flatten)]
    granted: Granted,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub(crate) struct Grantee {
    /// The local principal the grant is made to.
    #[arg(long, value_name = "NAME")]
    principal: Option<String>,
    /// The group the grant is made to.
    #[arg(long, value_name = "NAME")]
    group: Option<String>,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub(crate) struct Granted {
    /// A term, which says what the grantee may see. Repeatable: the terms are granted or revoked
    /// in one change, and one refused term refuses them all.
    #[arg(long = "term", value_name = "TERM")]
    terms: Vec<String>,
    /// A permission, which says what the grantee may do: `read`, `write`, `authorise-as`,
    /// `admin`, `read-all` or `write-all`. With `write`, the grantee writes only within its terms;
    /// `write-all` writes everywhere and is needed to declare and drop.
    #[arg(long, value_name = "NAME")]
    permission: Option<String>,
}

pub(crate) fn grant(args: GrantArgs, revoke: bool) -> ExitCode {
    let mut body = json!({});
    match (args.grantee.principal, args.grantee.group) {
        (Some(p), _) => body["principal"] = p.into(),
        (None, g) => body["group"] = g.into(),
    }
    match args.granted.permission {
        Some(p) => body["permission"] = p.into(),
        None => body["terms"] = args.granted.terms.into(),
    }
    let path = if revoke {
        "/control/grants/revoke"
    } else {
        "/control/grants"
    };
    let verb = if revoke { "revoke-grant" } else { "grant" };
    finish(verb, args.control.call("POST", path, Some(body)))
}

#[derive(Subcommand)]
pub(crate) enum ProviderCommand {
    /// List every OIDC provider, declared through the API or in `mosaica.toml`.
    List {
        #[command(flatten)]
        control: Control,
    },
    /// Show one provider.
    Show {
        #[command(flatten)]
        control: Control,
        /// The provider's name.
        name: String,
    },
    /// Declare an OIDC provider, or replace one whole. Every session authorised through a
    /// provider it replaces ends.
    Put {
        #[command(flatten)]
        control: Control,
        /// The provider's name.
        name: String,
        /// The `iss` its tokens carry.
        #[arg(long)]
        issuer: String,
        /// The `aud` its tokens must hold.
        #[arg(long)]
        audience: String,
        /// Where its signing keys are published: `https`, or `http` to a loopback address.
        #[arg(long, value_name = "URL")]
        jwks_url: String,
        /// A claim rule: a claim path and a template, such as `groups[*] {value}`. Repeatable.
        #[arg(long = "claim-rule", num_args = 2, value_names = ["CLAIM", "TEMPLATE"])]
        claim_rules: Vec<String>,
        /// A role mapping: a claim path, the exact value, and the local group it gives, such as
        /// `groups[*] mosaica-admins admins`. Repeatable.
        #[arg(long = "role-mapping", num_args = 3, value_names = ["CLAIM", "VALUE", "GROUP"])]
        role_mappings: Vec<String>,
    },
    /// Remove a provider. Every session authorised through it ends.
    Delete {
        #[command(flatten)]
        control: Control,
        /// The provider's name.
        name: String,
    },
}

pub(crate) fn provider(command: ProviderCommand) -> ExitCode {
    let at = |name: &str| format!("/control/providers/{}", segment(name));
    let answer = match command {
        ProviderCommand::List { control } => control.call("GET", "/control/providers", None),
        ProviderCommand::Show { control, name } => control.call("GET", &at(&name), None),
        ProviderCommand::Put {
            control,
            name,
            issuer,
            audience,
            jwks_url,
            claim_rules,
            role_mappings,
        } => {
            let rules: Vec<Value> = claim_rules
                .chunks(2)
                .map(|r| json!({ "claim": r[0], "template": r[1] }))
                .collect();
            let mappings: Vec<Value> = role_mappings
                .chunks(3)
                .map(|m| json!({ "claim": m[0], "value": m[1], "group": m[2] }))
                .collect();
            control.call(
                "PUT",
                &at(&name),
                Some(json!({
                    "issuer": issuer,
                    "audience": audience,
                    "jwks_url": jwks_url,
                    "claim_rules": rules,
                    "role_mappings": mappings,
                })),
            )
        }
        ProviderCommand::Delete { control, name } => control.call("DELETE", &at(&name), None),
    };
    finish("provider", answer)
}
