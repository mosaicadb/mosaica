# Connecting and creating

Use `connect` to read a database that someone else runs. It takes the token you read with, which decides what you may see, though a server started with `mosaica serve --open` needs none. If the operator has not issued you a token, `login` makes one from your own password, API key or OIDC access token. A back end that signs in its own users calls `authorise` instead, which makes a token for any local principal, OIDC identity or set of access terms, or the operator's own token.

Use `create` or `open` when the database is your own. Each gives a `Database` kept in a directory, which starts its own server when a commit or a read first needs it.

::: mosaica._viewer.connect

::: mosaica._database.create

::: mosaica._database.open

::: mosaica._database.Database
    options:
      members: false

::: mosaica._database.Database.binary
    options:
      heading_level: 3

::: mosaica._database.Database.save
    options:
      heading_level: 3

::: mosaica._database.Database.close
    options:
      heading_level: 3

::: mosaica._database.Database.token
    options:
      heading_level: 3

::: mosaica._database.Database.revoke
    options:
      heading_level: 3

::: mosaica._database.Database.viewer_url
    options:
      heading_level: 3

::: mosaica._database.Database.session_url
    options:
      heading_level: 3

::: mosaica._database.Database.operator_credential
    options:
      heading_level: 3

::: mosaica._auth.Token

::: mosaica._auth.TokenSource

::: mosaica._auth.login

::: mosaica._auth.logout

::: mosaica._auth.authorise

::: mosaica._auth.revoke

::: mosaica._refusal.Refusal
