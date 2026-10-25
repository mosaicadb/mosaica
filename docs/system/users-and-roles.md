# Users and roles

Every request to Mosaica is made as someone, whether that's a person reading the map, your app's server or a script loading data. It may be a principal, one of the accounts Mosaica keeps in its catalogue, or an identity your OpenID Connect provider vouches for, or the operator, Mosaica's one built-in superuser. This chapter covers what each of them holds and how each proves who it is. [Access control](access-control.md) explains how what they hold meets the labels on items.

## Who can make a request

Local principals are the accounts you create, and Mosaica keeps them in its catalogue, a SQLite database in the directory `[catalogue] dir` names in `mosaica.toml`. Each has a name and a kind, `person` or `service`, though the kind is only a label for your own records and changes nothing about what the principal may do. If you disable a principal, its sessions end and none of its credentials is accepted until you enable it again. Deleting one removes its password, its keys, its grants and its group memberships along with it.

If your organisation already has an OpenID Connect provider, its users don't need accounts in Mosaica. Mosaica stores nothing about these OIDC identities, and reads who each is, and what they hold, from the access token they present, through rules you declare for the provider.

The operator needs no account either. It proves itself with the operator credential, from the file or environment variable `mosaica.toml` names, and holds every permission. Because it isn't in the catalogue, a new deployment with an empty catalogue still has an administrator. To change the credential, change the file or the variable and restart Mosaica.

The catalogue lives outside the bundle, so principals and their grants carry across a rebuild. Mosaica holds the whole of it in memory, and a change you make through the control address takes effect at once, with no restart.

## What a principal holds

A principal holds terms, which decide what it may see, and permissions, which decide what it may do. A term is just a string, such as `cs.LG` or `team:geology`, and a principal may see an item when its terms satisfy the item's access label, as [access control](access-control.md#access-labels) describes. Permissions apply to the whole database, and you grant each one on its own:

| Permission | Allows |
|---|---|
| `read` | Starting a session for itself, and reading within its terms. A principal another acts as must hold it too |
| `read-all` | With `read`, a session that sees every item and satisfies every label |
| `write` | Inserting, deleting, suppressing and annotating within its own terms, flushes and compactions |
| `write-all` | With `write`, writing anywhere, and declaring and dropping views, fields, vocabularies and layers |
| `authorise-as` | Starting a session for another principal, at the session address |
| `admin` | Reading and changing the catalogue, listing and ending sessions, and reading the server's state at `/control/status` |

`admin` implies neither `read` nor `write`, so an account that manages users can be one that sees nothing. `read-all` and `write-all` widen `read` and `write` without implying them.

You give a principal terms or a permission by granting them, to the principal itself or to a group it belongs to, and take them away by revoking them. You can grant several terms in one change, and if any of them is refused, none is granted. A grant or revocation that changes something ends the sessions it affects, so that each picks up the change when it starts again; granting what's already held, or revoking what isn't, changes nothing and ends nothing.

## Groups

When several principals should see the same things, put them in a group and grant to the group. Each principal then holds what's granted to it directly, together with what's granted to every group it belongs to. Groups don't nest.

OIDC identities can't be members of a group, but a provider's role mappings give them a group's grants, as the section on providers describes. Adding or removing a member ends only that principal's sessions, while changing what a group holds ends the sessions of every member, and of every identity a role mapping gives the group to.

## Credentials

| Credential | Belongs to | Accepted at |
|---|---|---|
| A password | A local principal | `POST /v1/login`, to start a session |
| An API key | A local principal, which can have any number | `POST /v1/login`; the session address, where its principal holds `authorise-as`; the control address |
| An OIDC access token | An OIDC identity | `POST /v1/login`; `POST /session/authorise`, to name the identity a session is for; the control address |
| The operator credential | The operator | The session address and the control address |

API keys are what scripts and services use. A key looks like `msk_92ce602a0958_c37c…`: the twelve hex digits after `msk_` are a public prefix, by which you list and revoke it, and the 64 after the next underscore are the secret. Mosaica shows the whole key once, when it makes it, and keeps only a hash of the secret. You can give a key an expiry, at which every session started with it ends, and you can limit the permissions it holds. A limited key holds only those of its permissions its principal also holds, so a principal that both administers and writes can give a loading script a key that only writes. Revoking a key ends every session started with it.

A session token is none of these credentials. It's what a credential gets you when it starts a session, and it only reads, at the viewer address. The [access control chapter](access-control.md#getting-a-token) describes how each credential starts one, and how an access token is checked against its provider's published keys, issuer, audience and expiry.

??? info "Password limits"
    Passwords are at least 15 characters by default, set by `min_password_length`. After 10 failed attempts for one name within 15 minutes, further attempts for that name are refused as a wrong password is, until the oldest leaves the window. `failed_attempt_limit` and `failed_attempt_window` under `[catalogue]` set both.

## OIDC providers

To accept your provider's users, you tell Mosaica whose access tokens to trust and how to turn a token's claims into terms and groups. You can declare the provider in `mosaica.toml`, where it's fixed until you edit the file and restart, or through the control address, where an administrator can change it while Mosaica runs.

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

So an identity's terms come from its claim rules and from the groups its role mappings name, but its permissions come only from those groups. That means an identity can't start a session until a role mapping gives it a group that holds `read`.

## Sessions

A session keeps the terms and permissions its principal or identity had when it started. Rather than update an open session, Mosaica ends it when the catalogue changes in a way that could affect it, and the client starts a new one; [when a session ends](access-control.md#when-a-session-ends) lists every case. As an administrator, you can list the live sessions, all of them or one principal's or provider's, and end them by `token_id`, by principal or by provider.

## Managing the catalogue

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

When your server starts a session for a user, the session holds that user's terms, `read` and `write`, but never `read-all`, `write-all`, `admin` or `authorise-as`, even for a user who holds them. Your server's key can therefore read whatever any one user's terms admit, but it can't read everything through a user who holds `read-all`. If you need a session that reads everything through the session address, ask for one with the operator credential.

[Call Mosaica from a backend](../guides/backend.md) sets up the server's role, and a user for it to act as, step by step.

## Not built yet

- **OIDC sign-in in the clients.** Mosaica checks access tokens. Your app runs the provider's sign-in and passes the token on.
