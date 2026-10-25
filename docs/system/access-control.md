# Access control

Each item carries an access label, and each viewer's session holds a set of terms. A viewer may see the items whose labels their terms satisfy, and every answer they get is computed from those items alone: the points, the counts, the densities, the topics and the breakdowns. This chapter covers how labels are written, where a viewer's terms come from, how a session starts and ends, and what happens to an open session as the data changes. The [security](security.md) chapter sets out what a viewer can and cannot learn.

## Access labels

A label is an expression over terms, with `&` for "and" and `|` for "or":

| Label | Admits a viewer holding |
|---|---|
| `public` | anything: every session holds `public` |
| `team_a` | `team_a` |
| <code>team_a&#124;team_b</code> | either term |
| <code>secret&amp;(team_a&#124;team_b)</code> | `secret`, and either `team_a` or `team_b` |
| <code>user:alice&#124;user:bob</code> | either term, such as for a document shared with two people |

An item can carry a list of labels, and admits a viewer who satisfies any one of them. An item given no label takes its view's default, `point_visibility.default`. On a view with no default, an item with no label is refused.

Labels also gate more than items. A view, a view group, an annotation layer and each artifact can carry a label of its own, and a viewer who does not satisfy it is answered as if it did not exist. These labels are checked against the viewer's terms directly, so one can name a term no item carries.

Mosaica stores each label in a canonical form, so equivalent labels are the same label: `b&(a)` is stored as `a&b`, and `a|(a&b)` as `a`. The same parser reads labels at a build and at a running server.

??? info "The rules for writing a label"
    - A term written bare holds ASCII letters, digits and `_ - . : /`. Any other term is written in double quotes, with `\"` and `\\` as escapes, so `"team a"` holds a space.
    - Mixing `&` and `|` needs brackets: `a&b|c` is refused, and `(a&b)|c` is accepted.
    - Whitespace outside quotes is ignored. Two terms with only whitespace between them are refused.
    - `public` is reserved, and may only be the whole label. A term equal to `public` or `inherited` in any case, or holding a control character, is refused. So is a quoted term with whitespace at either end, since a viewer's terms are trimmed and could never hold it.
    - There is no negation.
    - A label that does not parse refuses the build, or the request that carries it, naming the label.

## Where a viewer's terms come from

A viewer's terms and permissions come from the identity catalogue, which [Users and roles](users-and-roles.md) describes. In brief:

- A local principal holds the terms and permissions granted to it and to each group it belongs to.
- An OIDC identity holds the terms its provider's claim rules produce from its token, and the terms and permissions of each group a role mapping names.
- Terms say what a principal may see. Permissions say what it may do: `read`, `read-all`, `write`, `write-all`, `authorise-as` and `admin`.

Writes go to the control address with the operator credential, an API key or an OIDC access token. A session token is for reading only.

## Getting a token

A session starts in one of three ways, and each gives the viewer a token: a random string that carries nothing of its own. The terms it stands for stay on the server.

- **The viewer logs in.** `POST /v1/login`, at the viewer address, takes a password, an API key or an OIDC access token. The session holds the principal's terms, and `read-all` where the principal has it.
- **Your server starts a session for the viewer.** `POST /session/authorise`, at the session address, takes an API key whose principal holds `authorise-as`, and names the principal to act as: a local one by name, or an OIDC identity by its access token. The session holds that principal's terms, and never its `read-all`. The session address takes a key that can act as any viewer, so it is a listener of its own, reached only by your server.
- **The operator names the terms.** With the operator credential, the session address can also start a session holding exactly a list of terms, belonging to no principal, or a session that sees everything.

One token covers every view the deployment serves. A refused bearer credential is answered with the same `401`, whatever the reason.

??? info "How credentials are checked"
    - **Passwords** are checked with argon2id. Failed attempts are limited per name, and password checks run about one per core at a time. Past that, they are answered `429`, whether or not the name exists.
    - **API keys** are found by their public prefix, and their secret compared by SHA-256 in constant time.
    - **OIDC access tokens** are checked against the keys their provider publishes, then their issuer, audience, `exp` and `nbf`. Only asymmetric algorithms are accepted. Declare the provider's audience as the API's, so an ID token, whose audience is the client's, is refused. A provider's keys are fetched again every hour; if that fails, the keys held are used for up to 24 hours.

Mosaica checks OIDC access tokens. It does not run a sign-in itself: your application runs the provider's sign-in page and passes the token on. Not built yet: OIDC sign-in in the clients.

## When a session ends

A session ends at the first of these:

- `token_max_lifetime` after it started;
- the expiry of the API key it was started with, or the `exp` of its access token;
- a change to the catalogue that could change what it may see or do: a grant or a membership changed, the principal disabled or deleted, a password set or cleared, its key revoked, its provider changed or dropped, or a change to a group one of the provider's role mappings names. A change that widens access ends sessions too, so they pick it up;
- for a session your server started, the same changes to the principal that started it, so a change to the integrator's own principal ends every session it started;
- `POST /v1/logout`, `POST /session/revoke`, or an administrator ending it;
- a restart, since sessions are held only in memory.

A request with an ended session's token is answered `403 expired-token`, or `401` once the server has forgotten the session, and after a restart. Either way, the client starts a new session.

## When the data changes under an open session

| Change | What an open session sees |
|---|---|
| An item is deleted or suppressed, or a suppression is lifted | The change applies from the session's next request |
| New items under terms the session holds are flushed | They appear once the server has brought the session's state up to date, usually by its next request. Until then it is answered from the previous version |
| The session's own access changes in the catalogue | The session ends |
| A view or view group is declared | It is unknown to the session until it starts again |

!!! note "Terms new to the database need a new session"
    A session sees an item only under terms and conjunctions the database already held when the session started. If a flush brings in a term the session holds but no item carried before, the session sees those items only once it starts again. It sees fewer items than its terms admit, never more, and nothing tells the client. A session that sees everything is not affected.

## How labels are indexed

Access control does its work once per session:

- Each term has a list of the items that carry it. An item's labels are read as one "or", and each part is listed on its own: `a|(b&c)` is listed under `a` and under `b&c`.
- When a session starts, the server works out which lists its terms satisfy, including every conjunction, which are kept in one shared graph so the check visits only what the session's terms reach. On the session's first read, it unions those lists into one bitmap of what the session may see.

Sessions whose terms satisfy the same lists share one result, kept in memory and on disc, so most sessions do not need to build the union again. Labels that only join terms with `|`, as per-user sharing does with labels such as `user:alice|user:bob`, are lists alone and add nothing to the graph. Each user then has a grant of their own, whose first request pays the cost [serving](serving.md#what-the-server-keeps-ready) describes.

A label with a conjunction may hold at most 1,024 nodes, counting each operator and each term once after duplicates are removed, so `a&b` is 3. An item is expected to fall under at most 4,096 lists, one for each part of its labels read as one "or". One with more is listed under all of them and reported, since dropping one would change who can see it.

## Not built yet

- **An audit log** of sessions started, credentials refused and catalogue changes.
- **Adding newly seen terms to an open session**, or telling the client that it is behind.
- **OIDC sign-in in the clients.**
