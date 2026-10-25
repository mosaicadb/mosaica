# Users and roles

Every request to Mosaica is made as someone: a person reading the map, your app's server starting sessions for its users, a script loading data, an administrator managing accounts. This chapter covers who those can be, what each holds, and how each proves who it is. [Access control](access-control.md) covers the other half: the labels on items, and the sessions that read them.

=== "HTTP"

    | To manage | Routes, on the control address |
    |---|---|
    | Principals | `GET`, `POST /control/principals`; `GET`, `PATCH`, `DELETE /control/principals/{name}` |
    | Passwords | `PUT`, `DELETE /control/principals/{name}/password` |
    | API keys | `GET`, `POST /control/principals/{name}/keys`; `DELETE /control/keys/{prefix}` |
    | Groups and members | `GET`, `POST /control/groups`; `GET`, `DELETE /control/groups/{name}`; `PUT`, `DELETE /control/groups/{name}/members/{principal}` |
    | Grants | `POST /control/grants`, `POST /control/grants/revoke` |
    | OIDC providers | `GET /control/providers`; `GET`, `PUT`, `DELETE /control/providers/{name}` |
    | Sessions | `GET /control/sessions`, `POST /control/sessions/end` |

=== "Python"

    | To manage | `Control` methods |
    |---|---|
    | Principals | `list_principals`, `show_principal`, `create_principal`, `change_principal`, `delete_principal` |
    | Passwords | `set_password`, `clear_password` |
    | API keys | `list_keys`, `create_key`, `revoke_key` |
    | Groups and members | `list_groups`, `show_group`, `create_group`, `delete_group`, `add_member`, `remove_member` |
    | Grants | `grant`, `revoke_grant` |
    | OIDC providers | `list_providers`, `show_provider`, `put_provider`, `drop_provider` |
    | Sessions | `list_sessions`, `end_sessions` |

=== "TypeScript"

    | To manage | `Control` methods |
    |---|---|
    | Principals | `listPrincipals`, `showPrincipal`, `createPrincipal`, `changePrincipal`, `deletePrincipal` |
    | Passwords | `setPassword`, `clearPassword` |
    | API keys | `listKeys`, `createKey`, `revokeKey` |
    | Groups and members | `listGroups`, `showGroup`, `createGroup`, `deleteGroup`, `addMember`, `removeMember` |
    | Grants | `grant`, `revokeGrant` |
    | OIDC providers | `listProviders`, `showProvider`, `putProvider`, `dropProvider` |
    | Sessions | `listSessions`, `endSessions` |

=== "CLI"

    | To manage | Commands |
    |---|---|
    | Principals | `mosaica principal list`, `show`, `create`, `disable`, `enable`, `delete` |
    | Passwords | `mosaica principal set-password`, `clear-password` |
    | API keys | `mosaica key list`, `create`, `revoke` |
    | Groups and members | `mosaica group list`, `show`, `create`, `delete`, `add-member`, `remove-member` |
    | Grants | `mosaica grant`, `mosaica revoke-grant` |
    | OIDC providers | `mosaica provider list`, `show`, `put`, `delete` |
    | Sessions | `mosaica session list`, `end` |

Each of these needs the `admin` permission, and goes to the control address with the operator credential, an API key or an OIDC access token.

## Who can make a request

- **Local principals** are the accounts Mosaica keeps itself, in the catalogue: a SQLite database in the directory `[catalogue] dir` names in `mosaica.toml`. Each has a name and a kind, `person` or `service`. The kind is a label for your own records and changes nothing about what the principal may do. A disabled principal's sessions end and none of its credentials is accepted until it is enabled again. Deleting a principal removes its password, its keys, its grants and its group memberships with it.
- **OIDC identities** are people or services your OpenID Connect provider knows. Mosaica stores nothing about them. It reads who they are, and what they hold, from each access token, through the rules declared for the provider.
- **The operator** is a built-in superuser, which proves itself with the operator credential from the file or environment variable `mosaica.toml` names. It holds every permission. It is not in the catalogue, so a new deployment with an empty catalogue still has an administrator. To change the credential, change the file and restart the server.

The catalogue lives outside the bundle, so principals and their grants carry across a rebuild. The server holds the whole of it in memory, and a change through the control address takes effect at once, with no restart.

## What a principal holds

A principal holds **terms**, which say what it may see, and **permissions**, which say what it may do.

A term is a string, such as `cs.LG` or `team:geology`. An item is visible to a principal when the principal's terms satisfy the item's access label, which [access control](access-control.md#access-labels) describes.

The permissions are over the whole database, and each is granted on its own:

| Permission | Allows |
|---|---|
| `read` | Starting a session for itself, and reading within its terms. A principal another acts as must hold it too |
| `read-all` | With `read`, a session that sees every item and satisfies every label |
| `write` | Inserting, deleting, suppressing and annotating within its own terms, flushes and compactions |
| `write-all` | With `write`, writing anywhere, and declaring and dropping views, fields, vocabularies and layers |
| `authorise-as` | Starting a session for another principal, at the session address |
| `admin` | Changing the catalogue, and listing and ending sessions |

`admin` implies neither `read` nor `write`, so an account that manages users can be one that sees nothing. `read-all` and `write-all` widen `read` and `write` without implying them.

A grant gives one term or one permission to a principal or to a group, and revoking it takes it away. A grant already held changes nothing. Any other grant or revocation ends the sessions it affects, so that each picks up the change when it starts again.

## Groups

A group is a set of local principals that hold grants together. A principal holds the terms and permissions granted to it and to every group it belongs to. Groups hold principals, not other groups.

An OIDC identity is a member of no group, but a provider's role mappings give it a group's grants, as the next section describes. Adding a member, removing one or changing a group's grants ends the sessions of everyone affected, including the identities a role mapping gives the group to.

## Credentials

| Credential | Belongs to | Accepted at |
|---|---|---|
| A password | A local principal | `POST /v1/login`, to start a session |
| An API key | A local principal, which can have any number | `POST /v1/login`; the session address, where its principal holds `authorise-as`; the control address |
| An OIDC access token | An OIDC identity | `POST /v1/login`; `POST /session/authorise`, to name the identity a session is for; the control address |
| The operator credential | The operator | The session address and the control address |

- **Passwords** are at least 15 characters by default, set by `min_password_length`. After 10 failed attempts for one name within 15 minutes, further attempts for that name are refused as a wrong password is, until the oldest leaves the window. `failed_attempt_limit` and `failed_attempt_window` under `[catalogue]` set both.
- **API keys** look like `msk_92ce602a0958_c37c…`. The part after `msk_` is a public prefix, by which the key is listed and revoked, and the rest is a secret, which Mosaica shows once, when it makes the key, and stores only as a hash. A key can have an expiry, and every session started with it ends then. A key can also name the permissions it holds, and then holds only those its principal also holds, so a principal that administers and writes can give a loading script a key that only writes. Revoking a key ends every session started with it.
- **Access tokens** are checked against the provider's published keys, issuer, audience and expiry, as [Access control](access-control.md#getting-a-token) describes.

A session token is none of these. It only reads, at the viewer address, and the [access control chapter](access-control.md#getting-a-token) says how each credential starts one.

## OIDC providers

A provider tells Mosaica whose access tokens to accept, and how to turn a token's claims into terms and groups. Declare one in `mosaica.toml`, where it cannot be changed through the control address, or through the control address, where an administrator can change it while the server runs.

```toml
[[catalogue.providers]]
name     = "staff"
issuer   = "https://login.example.org"
audience = "mosaica"
jwks_url = "https://login.example.org/.well-known/jwks.json"
claim_rules   = [{ claim = "groups[*]", template = "team:{value}" }]
role_mappings = [{ claim = "roles[*]", value = "map-reader", group = "readers" }]
```

- **`audience`** is the `aud` a token must carry. Name the API, so that an ID token, whose audience is your app's client id, is refused.
- **A claim rule** produces a term from each value a claim holds. Here, a token whose `groups` claim lists `geology` and `survey` gives the terms `team:geology` and `team:survey`. A claim path is a list of keys separated by dots, `[*]` takes every element of an array, and a key holding a dot or a bracket is written in double quotes.
- **A role mapping** gives every identity whose claim holds one exact value the terms and permissions of a local group. Here, an identity with the role `map-reader` gets whatever the group `readers` holds.

An identity's terms are those its claim rules produce, together with those of each group its role mappings name. Its permissions come only from those groups. An identity needs a role mapping to a group that holds `read` before it can start a session.

## Sessions

A session holds the terms and permissions its principal or identity had when it started. A change to the catalogue that could change them ends the session, and the client starts a new one. [When a session ends](access-control.md#when-a-session-ends) lists every case. An administrator can list the live sessions, all of them or one principal's or provider's, and end them by `token_id`, by principal or by provider.

## Typical roles

| Role | Principal | Grants |
|---|---|---|
| A viewer who sees part of the data | A person | `read`, and the terms for what they may see |
| A viewer who sees everything | A person | `read` and `read-all` |
| A team | A group, with each person a member | `read` and the team's terms, on the group |
| Your app's server, which starts sessions for its users | A service, with an API key | `authorise-as` |
| A script that loads data within some labels | A service, with an API key | `write`, and the terms it writes under |
| A script that loads anything, and declares | A service, with an API key | `write` and `write-all` |
| An administrator | A person or a service | `admin`, and `read` if they also read |

A session your server starts for a user holds that user's terms, `read` and `write`, and never `read-all`, `write-all`, `admin` or `authorise-as`, even for a principal that holds them. To read everything through the session address, the operator credential asks for a session that does.

[Call Mosaica from a backend](../guides/backend.md) sets up the server's role, and a user for it to act as, step by step.

## Not built yet

- **OIDC sign-in in the clients.** Mosaica checks access tokens. Your app runs the provider's sign-in and passes the token on.
