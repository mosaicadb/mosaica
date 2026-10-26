"""Tokens: what a reader presents to read, and the functions that make and end one.

A reader holds a token its deployment issued, or makes one with `login` from its own password, API
key or OIDC access token. A back end that signs in its own users calls `authorise` instead, which
makes a token for any local principal, OIDC identity or set of access terms, or the operator's own
token.
"""

from __future__ import annotations

import json
import time
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from typing import Callable, Optional, Sequence, Union

from ._refusal import Refusal, listed


@dataclass
class Token:
    """A token that lets its holder read a Mosaica database until its session ends.

    `login` and `authorise` return one, and `connect`, `Map` and `logout` take one or its `token`
    string. Printing a token shows its expiry, its principal and how many terms it holds, and
    leaves the token string out, so that the secret does not end up in a saved notebook or a log.

    - `token`: the secret string a reader sends with each request. Anyone who holds it reads as
      the token's principal.
    - `expires_at`: when the token's session ends, in seconds since 1970, or `None` when that is
      not known. A catalogue change, such as a grant to its principal, can end it sooner.
    - `token_id`: the id `revoke` takes to end the token without sending the token again. Only a
      token `authorise` made has one, and it is `None` otherwise.
    - `renew`: a function that makes a fresh token from the credential that made this one, which
      therefore stays in this process. It is set when `login` or `authorise` made the token and
      is `None` otherwise. A reader from `connect` and a `Map` call it when the token is about to
      expire, or when a `403 expired-token` answer says that its session has ended.
    - `principal`: the local principal the token reads as, when `authorise` made it for one or
      `login` was given a password, and `None` otherwise.
    - `terms`: the access terms the token holds, when `authorise` made it for a set of terms, and
      `None` otherwise.
    """

    token: str
    expires_at: Optional[float] = None
    token_id: Optional[int] = None
    renew: Optional[Callable[[], "Token"]] = field(default=None, repr=False, compare=False)
    principal: Optional[str] = field(default=None, compare=False)
    terms: Optional[Sequence[str]] = field(default=None, repr=False, compare=False)

    @property
    def seconds_left(self) -> Optional[float]:
        """Seconds until the token expires, or `None` if its expiry is not known."""
        return None if self.expires_at is None else self.expires_at - time.time()

    def __repr__(self) -> str:
        # Never the token: a printed token ends up in saved notebooks and logs.
        left = "" if self.seconds_left is None else f", {self.seconds_left:.0f}s left"
        who = "" if self.principal is None else f", principal={self.principal!r}"
        holding = "" if self.terms is None else f", {len(self.terms)} term(s)"
        return f"Token(expires_at={self.expires_at}{left}{who}{holding})"


TokenSource = Union[str, Token, Callable[[], Union[str, "Token"]]]
"""The forms in which `connect`, `Viewer` and `Map` take a token: the token string, a `Token`, or a
function that returns either.

A reader gets a fresh token when the one it holds is about to expire, or when a `403
expired-token` answer says that its session has ended. It renews a `Token` with the token's own
`renew` and calls a function again, while it sends a plain string unchanged until the server
refuses it. A function is therefore the way to hand a reader tokens that your own code makes."""


def minted(source: TokenSource) -> Token:
    """A `Token` from any of the forms a token may be given in."""
    got = source() if callable(source) and not isinstance(source, Token) else source
    if isinstance(got, str):
        got = Token(got)
    if not isinstance(got, Token) or not got.token:
        raise Refusal(
            f"a token must be a string, a Token or a callable returning one; got {got!r}"
        )
    return got


def request_headers(method: str, bearer: Optional[str], given: Optional[dict] = None) -> dict:
    """A request's headers: the bearer where there is one, then `given`, then JSON as the content
    type of a write that names none, since an open server refuses a write with no content type,
    body or not."""
    head = {} if bearer is None else {"authorization": f"Bearer {bearer}"}
    head.update(given or {})
    if method != "GET" and "content-type" not in head:
        head["content-type"] = "application/json"
    return head


def _post(url: str, bearer: Optional[str], body: Optional[dict], timeout: float, verb: str):
    """One POST, answering the parsed JSON body, or `None` where there is none."""
    data = None if body is None else json.dumps(body).encode()
    request = urllib.request.Request(url, data=data, method="POST", headers=request_headers("POST", bearer))
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            text = response.read()
    except urllib.error.HTTPError as e:
        detail = e.read().decode(errors="replace")
        raise PermissionError(f"{verb} refused ({e.code}): {detail}") from None
    return json.loads(text) if text else None


def login(
    viewer_url: str,
    *,
    principal: Optional[str] = None,
    password: Optional[str] = None,
    api_key: Optional[str] = None,
    access_token: Optional[str] = None,
    timeout: float = 10.0,
) -> Token:
    """Log in to a Mosaica database and return a token that reads as the principal the
    credential belongs to.

    - `viewer_url`: the address of the database's viewer plane, where readers read.
    - `principal`, `password`: a local principal's name and password, given together.
    - `api_key`: an API key the database's operator issued.
    - `access_token`: an OIDC access token from a provider the database accepts.
    - `timeout`: how long to wait for the server, in seconds. The default is 10.

    Any combination but exactly one of the three credentials raises `ValueError`, as a missing
    `viewer_url` does. The principal must hold the `read` permission. The server answers
    every credential it does not accept with the same refusal, which raises `PermissionError`, so
    the error does not reveal whether a principal exists.

        token = mosaica.login(viewer_url, principal="ann", password=password)
        mosaica.connect(viewer_url, token).view("papers").count()
    """
    if not viewer_url:
        raise ValueError("login needs the viewer plane's URL")
    given = [
        principal is not None or password is not None,
        api_key is not None,
        access_token is not None,
    ]
    if sum(given) != 1 or (given[0] and (principal is None or password is None)):
        raise ValueError(
            "login takes exactly one credential: principal= with password=, api_key=, or "
            "access_token="
        )
    if given[0]:
        body: dict = {"password": {"principal": principal, "password": password}}
    elif api_key is not None:
        body = {"api_key": api_key}
    else:
        body = {"access_token": access_token}
    answer = _post(viewer_url.rstrip("/") + "/v1/login", None, body, timeout, "/v1/login")
    return Token(
        token=answer["token"],
        expires_at=float(answer["expires_at"]),
        renew=lambda: login(
            viewer_url,
            principal=principal,
            password=password,
            api_key=api_key,
            access_token=access_token,
            timeout=timeout,
        ),
        principal=principal,
    )


def logout(viewer_url: str, token: Union[str, Token], *, timeout: float = 10.0) -> None:
    """End a token's session, so that the token can no longer read.

    - `viewer_url`: the address of the database's viewer plane.
    - `token`: the token to end, as a string or a `Token`.
    - `timeout`: how long to wait for the server, in seconds. The default is 10.

    A token the server does not recognise, or whose session has already ended, raises
    `PermissionError`.
    """
    if not viewer_url:
        raise ValueError("logout needs the viewer plane's URL")
    bearer = token.token if isinstance(token, Token) else token
    _post(viewer_url.rstrip("/") + "/v1/logout", bearer, None, timeout, "/v1/logout")


def authorise(
    session_url: str,
    credential: str,
    *,
    principal: Optional[str] = None,
    access_token: Optional[str] = None,
    terms: Optional[Sequence[str]] = None,
    read_all: bool = False,
    timeout: float = 10.0,
) -> Token:
    """Make a token on the session plane that reads as a local principal, an OIDC identity, a set
    of access terms or the operator.

    An application that signs its own users in calls this from its back end, with an API key, to
    give each user a token that reads as them.

    - `session_url`: the address of the database's session plane, where tokens are made.
    - `credential`: an API key whose principal holds `authorise-as`, or the operator credential.
      Either one can make a token that reads as anyone.
    - `principal`: the name of a local principal to read as. The token carries the principal's
      terms and its `read` and `write` permissions, and none of its others, such as `read-all`.
      The principal must hold `read`.
    - `access_token`: the OIDC access token of an identity to read as, which the server checks
      as `login` does. The token carries the identity's terms and permissions as for `principal`.
    - `terms`: the access terms the token holds, as a list or one term as a string, for a reader
      the catalogue does not name. The token holds them and `read`. Only the operator credential
      may send `terms`.
    - `read_all`: `True` makes the operator's own token, which holds `read-all` and so reads every
      item that is not deleted or suppressed, including items committed after the token was made.
      Only the operator credential may send it.
    - `timeout`: how long to wait for the server, in seconds. The default is 10.

    Name exactly one of `principal`, `access_token`, `terms` and `read_all`. Anything else, or an
    empty `session_url` or `credential`, raises `ValueError`, and a request the server refuses
    raises `PermissionError`, whose message holds the server's status and answer. A `Database`
    makes these tokens with its own operator credential through `db.token(terms)` and
    `db.viewer(terms)`.

        token = mosaica.authorise(session_url, api_key, principal="ann")
        token = mosaica.authorise(session_url, operator_credential, terms=["cs.LG"])
        mosaica.connect(viewer_url, token).view("papers").count()
    """
    if not credential:
        raise ValueError(
            "authorise needs a credential: an API key whose principal holds authorise-as, or the "
            "operator credential"
        )
    if not session_url:
        raise ValueError("authorise needs the session plane's URL")
    named = {
        "principal": principal,
        "access_token": access_token,
        "terms": None if terms is None else listed(terms),
        "read_all": True if read_all else None,
    }
    body = {k: v for k, v in named.items() if v is not None}
    if len(body) != 1:
        raise ValueError(
            "authorise takes exactly one of principal=, access_token=, terms= and read_all=True"
        )
    answer = _post(
        session_url.rstrip("/") + "/session/authorise",
        credential,
        body,
        timeout,
        "/session/authorise",
    )
    return Token(
        token=answer["token"],
        expires_at=float(answer["expires_at"]),
        token_id=int(answer["token_id"]),
        renew=lambda: authorise(
            session_url,
            credential,
            principal=principal,
            access_token=access_token,
            terms=terms,
            read_all=read_all,
            timeout=timeout,
        ),
        principal=principal,
        terms=named["terms"],
    )


def revoke(
    session_url: str,
    credential: str,
    token_id: Union[int, Token],
    *,
    timeout: float = 10.0,
) -> None:
    """End a token that `authorise` made, so that it can no longer read.

    - `session_url`: the address of the database's session plane.
    - `credential`: the operator credential, which ends any token, or an API key, which ends only
      the tokens made with a key of its own principal.
    - `token_id`: the `Token`, or its `token_id`. Only the id is sent.
    - `timeout`: how long to wait for the server, in seconds. The default is 10.

    The server accepts an id that names no live token without saying so, because any other
    answer would tell a caller which tokens exist. An empty `session_url` or `credential`, or a
    `Token` without a `token_id`, raises `ValueError`, and a request the server refuses raises
    `PermissionError`. A reader holding the `Token` can make another with its `renew()`, because
    the credential that made it keeps working.

        mosaica.revoke(session_url, credential, token)
    """
    if not credential:
        raise ValueError(
            "revoke needs a credential: an API key whose principal holds authorise-as, or the "
            "operator credential"
        )
    if not session_url:
        raise ValueError("revoke needs the session plane's URL")
    handle = token_id.token_id if isinstance(token_id, Token) else token_id
    if handle is None:
        raise ValueError(
            "revoke needs a token_id: this Token was not made by authorise and carries none. "
            "Pass the token_id /session/authorise returned, or end it with logout"
        )
    _post(
        session_url.rstrip("/") + "/session/revoke",
        credential,
        {"token_id": int(handle)},
        timeout,
        "/session/revoke",
    )
