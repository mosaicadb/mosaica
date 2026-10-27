# Access control

Mosaica controls what each viewer sees by attributes. You give each item an access label, and each viewer holds a set of access attributes, which their session carries. A viewer sees the items whose labels their attributes satisfy, and everything they get back is computed from those items alone: the counts, densities, topics and breakdowns as well as the points.

This chapter covers how you write labels, how a session starts and ends, and what an open session sees as the data changes. [Users and roles](users-and-roles.md) covers who holds which attributes, and [security](security.md) sets out what a viewer can and cannot learn.

## Access labels

A label is an expression over access attributes, which the API calls *terms*, with `&` for "and" and `|` for "or":

| Label | Admits a viewer holding |
|---|---|
| `public` | anything: every session holds `public` |
| `team_a` | `team_a` |
| <code>team_a&#124;team_b</code> | either attribute |
| <code>secret&amp;(team_a&#124;team_b)</code> | `secret`, and either `team_a` or `team_b` |
| <code>user:alice&#124;user:bob</code> | either attribute, such as for a document shared with two people |

An item can carry a list of labels, in which case a viewer who satisfies any one of them may see it. If you give an item no label, it takes its view's default, `point_visibility.default`, and on a view with no default an item with no label is refused.

You can label more than items. A view, a view group, an annotation layer and each artifact can carry a label of its own, and a viewer who doesn't satisfy it is answered as if it didn't exist. Because these labels are checked against the viewer's attributes directly, they can name an attribute that no item carries.

Whether a viewer reaches a view of a group depends on the group they look through and the view's own label in that group. A group that shares another's views with `members` has its own label, and a view reached through it takes that label alone, not the label the view carries in the group that owns it. The items in it keep their own labels, so an item that must stay hidden in every group carries the label itself.

Mosaica stores each label in a canonical form, so `b&(a)` is the same label as `a&b`, and `a|(a&b)` the same as `a`. A label reads the same whether it arrives in the first load or at a running database.

??? info "The rules for writing a label"
    - An attribute written bare holds ASCII letters, digits and `_ - . : /`. Any other is written in double quotes, with `\"` and `\\` as escapes, so `"team a"` holds a space.
    - Mixing `&` and `|` needs brackets: `a&b|c` is refused, and `(a&b)|c` is accepted.
    - Whitespace outside quotes is ignored. Two attributes with only whitespace between them are refused.
    - `public` is reserved, and may only be the whole label. An attribute equal to `public` or `inherited` in any case, or holding a control character, is refused. So is a quoted attribute with whitespace at either end, since a viewer's attributes are trimmed and could never hold it.
    - There is no negation.
    - A label that doesn't parse refuses the build, or the request that carries it, naming the label.

## Getting a token

Every session gives the viewer a token, a random string that carries nothing itself: the attributes it stands for stay inside Mosaica. One token covers every view the deployment serves. There are three ways to start a session.

The viewer can log in, with `POST /v1/login` at the viewer address, using a password, an API key or an OIDC access token. The session then holds the user's attributes, and `read-all` if they have it.

Or your own server starts the session for the viewer. It calls `POST /session/authorise` at the session address with an API key whose user holds `authorise-as`, naming the user to act for: a Mosaica user by name, or a provider's user by their access token. That session holds the user's attributes and never their `read-all`. Because the session address takes a key that can act as any viewer, it has a listener of its own, which only your server reaches.

Finally, the operator credential can start a session at the session address that holds exactly a list of attributes, belonging to no user, or one that sees everything.

Whatever the reason a bearer credential is refused, the answer is the same `401`.

??? info "How credentials are checked"
    - **Passwords** are checked with argon2id. Failed attempts are limited per name, and password checks run about one per core at a time. Past that, they are answered `429`, whether or not the name exists.
    - **API keys** are found by their public prefix, and their secret compared by SHA-256 in constant time.
    - **OIDC access tokens** are checked against the keys their provider publishes, then their issuer, audience, `exp` and `nbf`. Only asymmetric algorithms are accepted. Declare the provider's audience as the API's, so an ID token, whose audience is the client's, is refused. A provider's keys are fetched again every hour; if that fails, the keys held are used for up to 24 hours.

Mosaica checks OIDC access tokens, but it doesn't run a sign-in itself: your application runs the provider's sign-in page and passes the token on. Not built yet: OIDC sign-in in the clients.

## When a session ends

A session ends at the first of these:

- `token_max_lifetime` after it started;
- the expiry of the API key it was started with, or the `exp` of its access token;
- a change to the catalogue that could change what it may see or do: a grant or a membership changed, the user disabled or deleted, a password set or cleared, its key revoked, its provider changed or dropped, or a change to a group one of the provider's role mappings names. A change that widens access ends sessions too, so that they pick it up;
- for a session your server started, the same changes to your server's own service account;
- `POST /v1/logout`, `POST /session/revoke`, or an administrator ending it;
- a restart, since sessions are held only in memory.

A request with an ended session's token is answered `403 expired-token`, or `401` once Mosaica has forgotten the session, and after a restart. Either way, the client starts a new session.

## When the data changes under an open session

| Change | What an open session sees |
|---|---|
| An item is deleted or suppressed, or a suppression is lifted | The change applies from the session's next request |
| New items under attributes the session holds are flushed | They appear once Mosaica has brought the session's state up to date, usually by its next request. Until then it is answered from the previous version |
| The session's own access changes in the catalogue | The session ends |
| A view or view group is declared | It is unknown to the session until it starts again |

!!! note "Attributes new to the database need a new session"
    A session sees an item only under attributes and conjunctions that items' labels already used when the session started. If a flush brings in an attribute the session holds but no label used before, the session sees those items only once it starts again. It sees fewer items than its attributes admit, never more, and nothing tells the client. A session that sees everything is not affected.

## How labels are indexed

Access control does most of its work once per session, rather than on every request. Each attribute has a list of the items that carry it. An item's labels are read as one "or", and each part is listed on its own, so `a|(b&c)` is listed under `a` and under `b&c`.

When a session starts, Mosaica works out which lists its attributes satisfy, conjunctions included. The conjunctions are kept in one shared graph, so the check visits only what the session's attributes reach. Then, on the session's first read, it unions those lists into one bitmap of what the session may see.

Sessions whose attributes satisfy the same lists share that result, which is kept in memory and on disc, so most sessions don't build the union again. Labels that only join attributes with `|`, as per-user sharing does with labels such as `user:alice|user:bob`, are lists alone and add nothing to the graph. Each user then has a visible set of their own, whose first request pays the cost [serving](serving.md#what-the-server-keeps-ready) describes.

??? info "Limits on labels"
    A label with a conjunction may hold at most 1,024 nodes, counting each operator and each attribute once after duplicates are removed, so `a&b` is 3. An item is expected to fall under at most 4,096 lists, one for each part of its labels read as one "or". One with more is listed under all of them and reported, since dropping one would change who can see it.

## Not built yet

- **An audit log** of sessions started, credentials refused and catalogue changes.
- **Adding newly seen attributes to an open session**, or telling the client that it is behind.
- **OIDC sign-in in the clients.**
