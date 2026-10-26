# Connecting and creating

Use `connect` to read a database that someone else runs. It takes a token, which decides the access terms you read with, and a server started with `mosaica serve --open` needs none. The database's operator can issue you a token, `login` makes one from a password, an API key or an OIDC access token, and `authorise` makes one for a principal, a set of access terms or the operator.

Use `create` or `open` when the database is your own. Each gives a `Database` in a directory, which starts its own server at the first commit and makes its tokens with its own operator credential.

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
