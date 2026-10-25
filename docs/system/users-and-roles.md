# Users and roles

Mosaica checks two things on every request: what the caller may do, and what they may see.

- **What they may do** is role-based. Each permission, such as reading or writing, is granted to a group or directly to a user, and groups play the part of roles.
- **What they may see** is attribute-based. Each user holds a set of access attributes, such as `team:geology`, and each item carries an access label, such as `team:geology|public-data`. A user sees the items whose labels their attributes satisfy, and every count and cluster on their map is computed from those items alone.

This chapter covers users, permissions, attributes and groups, and how users sign in. [Access control](access-control.md) covers access labels and sessions.

## Users and service accounts

You create users in Mosaica's catalogue, either for people or as service accounts for your app's server and your scripts. The two kinds behave the same, and the difference is only for your own records. The API calls both *principals*.

If your organisation already has an OpenID Connect provider, its users don't need accounts in Mosaica. Mosaica reads who each one is from the access token they present, and gives them attributes and permissions through rules you declare for the provider, as [Single sign-on](#single-sign-on-with-openid-connect) describes.

Mosaica also has one built-in superuser, the operator, which holds every permission. It signs in with the operator credential, which `mosaica.toml` names as a file or an environment variable, so a new database with no users still has an administrator.

??? info "Where the catalogue is kept"
    The catalogue is a SQLite database in the directory `[catalogue] dir` names. It lives outside the database's data, so users and their grants survive a rebuild. Mosaica holds it in memory, and a change takes effect at once, with no restart. The operator credential is not in the catalogue: to change it, change the file or the variable and restart.

## Permissions

| Permission | Allows |
|---|---|
| `read` | Signing in and reading what the user's attributes admit |
| `read-all` | With `read`, reading every item, whatever its label |
| `write` | Inserting, deleting, suppressing and annotating items under the user's own attributes, and running flushes and compactions |
| `write-all` | With `write`, writing any item, and declaring and dropping views, fields, vocabularies and layers |
| `authorise-as` | Starting a session on behalf of another user, who must hold `read`. This is for your app's server |
| `admin` | Managing users, groups, keys, providers and sessions, and reading the server's state |

Each permission is granted on its own, and none implies another. An administrator who should also see the data needs `read` as well as `admin`.

## Access attributes

An access attribute is any string you choose, such as `team:geology`, `clearance:secret` or `user:alice`, and is called a *term* in the API. Grant a user the attributes for what they may see, and give items labels written over the same attributes. [Access labels](access-control.md#access-labels) shows how labels combine attributes with "and" and "or".

## Groups

When several users should have the same access, create a group, grant the permissions and attributes to the group, and add the users to it. A user holds everything granted to them directly and to each group they're in. Groups don't nest.

A change to what someone holds takes effect at their next session: Mosaica ends the sessions it affects, and the client starts new ones. Changing a group's grants ends the sessions of all its members, while adding or removing one member ends only that user's.

## Signing in

| Credential | Used by | Accepted for |
|---|---|---|
| A password | People | Signing in to read |
| An API key | Service accounts and scripts | Signing in to read, writing, administering, and starting sessions for other users |
| An OIDC access token | Your provider's users | Signing in to read, writing and administering |
| The operator credential | The operator | Writing, administering, and starting sessions |

Signing in to read gives the client a session token, which it sends with each request for the map. Writes and administration take the credential itself. [Getting a token](access-control.md#getting-a-token) describes each way to start a session.

??? info "API keys"
    A key looks like `msk_92ce602a0958_c37c…`. The twelve hex digits after `msk_` are a public prefix, by which you list and revoke the key, and the rest is the secret. Mosaica shows the whole key once, when it creates it, and keeps only a hash. A user can have any number of keys.

    You can give a key an expiry, and you can limit it to some of its user's permissions, so a user who both administers and writes can give a loading script a key that only writes. Revoking a key, or reaching its expiry, ends every session started with it.

??? info "Passwords"
    Passwords are at least 15 characters by default, set by `min_password_length`. After 10 failed attempts for one name within 15 minutes, further attempts for that name are refused until the oldest leaves the window. `failed_attempt_limit` and `failed_attempt_window` under `[catalogue]` set both.

## Single sign-on with OpenID Connect

To accept your provider's users, you tell Mosaica whose tokens to trust and how to turn a token's claims into attributes and groups. Declare the provider in `mosaica.toml`, or at the running database, where an administrator can change it without a restart:

```toml
[[catalogue.providers]]
name     = "staff"
issuer   = "https://login.example.org"
audience = "mosaica"
jwks_url = "https://login.example.org/.well-known/jwks.json"

# Each value of the groups claim becomes an attribute: geology → team:geology.
claim_rules   = [{ claim = "groups[*]", template = "team:{value}" }]

# A user with the role map-reader gets everything the group readers holds.
role_mappings = [{ claim = "roles[*]", value = "map-reader", group = "readers" }]
```

Set `audience` to the one your provider puts on access tokens for this API, so an ID token, which carries your app's client id instead, is refused.

Permissions come only through role mappings, so a provider's user can't sign in until a role mapping gives them a group that holds `read`. Attributes can come from claim rules, from those groups, or both.

??? info "Claim paths"
    A claim path is a list of keys separated by dots. `[*]` takes every element of an array, and a key holding a dot or a bracket is written in double quotes.

## Common setups

| Who | Account | Grants |
|---|---|---|
| A person who sees part of the data | A user | `read`, and the attributes for what they may see |
| A person who sees everything | A user | `read` and `read-all` |
| A team | A group, with each person a member | `read` and the team's attributes, on the group |
| Your app's server, which starts sessions for its users | A service account with an API key | `authorise-as` |
| A script that loads data under some labels | A service account with an API key | `write`, and the attributes it writes under |
| A script that loads anything and declares the schema | A service account with an API key | `write` and `write-all` |
| An administrator | A user or a service account | `admin`, and `read` if they also read |

A session your server starts for a user holds that user's attributes, `read` and `write`, but never `read-all`, `write-all`, `admin` or `authorise-as`. So your server can read whatever any one user may, but can't read everything through a user who holds `read-all`. [Call Mosaica from a backend](../guides/backend.md) sets this up step by step.

## Managing users

Every change needs the `admin` permission, and goes to the control address with the operator credential, an API key or an OIDC access token.

=== "HTTP"

    | To manage | Routes, on the control address |
    |---|---|
    | Users | `GET`, `POST /control/principals`; `GET`, `PATCH`, `DELETE /control/principals/{name}` |
    | Passwords | `PUT`, `DELETE /control/principals/{name}/password` |
    | API keys | `GET`, `POST /control/principals/{name}/keys`; `DELETE /control/keys/{prefix}` |
    | Groups and members | `GET`, `POST /control/groups`; `GET`, `DELETE /control/groups/{name}`; `PUT`, `DELETE /control/groups/{name}/members/{principal}` |
    | Grants | `POST /control/grants`, `POST /control/grants/revoke` |
    | OIDC providers | `GET /control/providers`; `GET`, `PUT`, `DELETE /control/providers/{name}` |
    | Sessions | `GET /control/sessions`, `POST /control/sessions/end` |

=== "Python"

    | To manage | `Control` methods |
    |---|---|
    | Users | `list_principals`, `show_principal`, `create_principal`, `change_principal`, `delete_principal` |
    | Passwords | `set_password`, `clear_password` |
    | API keys | `list_keys`, `create_key`, `revoke_key` |
    | Groups and members | `list_groups`, `show_group`, `create_group`, `delete_group`, `add_member`, `remove_member` |
    | Grants | `grant`, `revoke_grant` |
    | OIDC providers | `list_providers`, `show_provider`, `put_provider`, `drop_provider` |
    | Sessions | `list_sessions`, `end_sessions` |

=== "TypeScript"

    | To manage | `Control` methods |
    |---|---|
    | Users | `listPrincipals`, `showPrincipal`, `createPrincipal`, `changePrincipal`, `deletePrincipal` |
    | Passwords | `setPassword`, `clearPassword` |
    | API keys | `listKeys`, `createKey`, `revokeKey` |
    | Groups and members | `listGroups`, `showGroup`, `createGroup`, `deleteGroup`, `addMember`, `removeMember` |
    | Grants | `grant`, `revokeGrant` |
    | OIDC providers | `listProviders`, `showProvider`, `putProvider`, `dropProvider` |
    | Sessions | `listSessions`, `endSessions` |

=== "CLI"

    | To manage | Commands |
    |---|---|
    | Users | `mosaica principal list`, `show`, `create`, `disable`, `enable`, `delete` |
    | Passwords | `mosaica principal set-password`, `clear-password` |
    | API keys | `mosaica key list`, `create`, `revoke` |
    | Groups and members | `mosaica group list`, `show`, `create`, `delete`, `add-member`, `remove-member` |
    | Grants | `mosaica grant`, `mosaica revoke-grant` |
    | OIDC providers | `mosaica provider list`, `show`, `put`, `delete` |
    | Sessions | `mosaica session list`, `end` |

Disabling a user ends their sessions and refuses their credentials until you enable them again. Deleting one also removes their password, keys, grants and group memberships. A grant of several attributes at once is all or nothing: if one is refused, none is granted.

## Not built yet

- **OIDC sign-in in the clients.** Mosaica checks access tokens, but your app runs the provider's sign-in and passes the token on.
