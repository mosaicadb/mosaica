"""The control plane as the SDK calls it.

`Control` is the routes and nothing else: the SDK keeps no record of what it sent, so what a
database holds is asked of the database.

**A batch id is a fresh id per request, and a retry resends both the id and the bytes.** The id is
the client's choice and says which request this is; the server holds it against the SHA-256 of the
body, so a retry that re-serialised its table would be refused as a different body under a held
id. The bodies are therefore built once, by the plan, and this module sends the bytes it was
given. Two requests carrying the same rows are two requests: nothing derives an id from what
a page contains, so a frame inserted and committed twice lands twice.

**A `429` is backpressure.** Every `429` carries `Retry-After`, whatever shed the request, and
the client waits that long and sends the same bytes again. A page of rows no server answered is
sent once more, the same bytes under its batch id, which the server answers as a replay where the
first arrived; unanswered twice, it is the answer. Every other status is an answer: a `409` on a
differing part is reported and not retried, since an edit is a delete and a re-ingest, and the SDK
does not do that on the user's behalf.

The transport is `urllib`, so the SDK's only dependency outside the standard library stays
pyarrow.
"""

from __future__ import annotations

import json
import time
from time import sleep
import urllib.error
import urllib.parse
import urllib.request
import uuid
from dataclasses import dataclass, field

from ._auth import request_headers
from ._refusal import listed, retry_after
from typing import Any, Sequence

ARROW = "application/vnd.apache.arrow.stream"
JSON = "application/json"

#: The longest wait between attempts after a `429`, in seconds. A longer `Retry-After` is cut to
#: this.
MAX_BACKOFF = 30.0

#: The most one request waits in all between attempts after a `429`, in seconds. A `429` is waited
#: out and the request sent again while the waits stay within this; the `429` whose wait would take
#: them past it is the answer.
RETRY_BUDGET_S = 300.0

#: The status an answer carries when the request reached no server at all. Not an HTTP status: the
#: request was never answered, so there is none to report.
UNANSWERED = 0


def batch_id(source: str, index: int) -> str:
    """A page's batch id: a fresh random id, made once when the request is built.

    The id is never derived from the request's content. Retries of one attempt inside one
    `commit()`, after a `429`, a timeout or a lost answer, reuse it, and nothing else does.
    Nothing is kept across commits, so the same table inserted and committed five times is
    loaded five times.

    The source name and the page index ride in front of the random half so that an operator
    reading a server log can tell which page a line is about; nothing reads them.
    """
    return f"{source}-{index}-{uuid.uuid4().hex}"


@dataclass
class Answer:
    """The answer to one request a `Control` sent, whether the server accepted it, refused it or
    never answered.

    - `status`: the HTTP status, or 0 when no server answered.
    - `body`: the answer's JSON object. It is `{}` when the answer was not JSON, and a JSON value
      that is not an object is under `"value"`.
    - `detail`: the answer's text, or why no answer came.
    - `attempts`: how many times the request was sent, which is more than 1 when the server asked
      the client to wait with a `429`, or when an ingest that got no answer was sent again.
    - `seconds`: how long the request took, from its first attempt to its answer.
    """

    status: int
    body: dict = field(default_factory=dict)
    detail: str = ""
    attempts: int = 1
    seconds: float = 0.0

    @property
    def ok(self) -> bool:
        """`True` when the status is in the 200s, which means the server accepted the request."""
        return 200 <= self.status < 300


class Control:
    """A client for the control plane of a served database, where the operator declares, writes,
    deletes and manages who may read.

    `Database.control` returns one for a database you made, and `Control(base, credential)` makes
    one for a database someone else runs. Each route method sends one HTTP request and returns its
    `Answer` without raising, so a refusal is read from the answer's `status` and `detail`. When a
    busy server answers `429`, the method waits as long as the server asks, up to 30 seconds, and
    sends the request again. It returns the `429` once the waits would pass five minutes in all.

    - `base`: the control plane's address, such as `http://127.0.0.1:41234`.
    - `credential`: the operator credential, an API key or an OIDC access token, or `None` for a
      server started with `mosaica serve --open`, which needs none. A method that writes, flushes or
      compacts needs the `write` permission, one that declares or drops needs `write-all` as well,
      and `status` and the catalogue methods need `admin`. A credential that holds `write` without
      `write-all` can name only what its terms let it see.
    - `timeout`: how long to wait for one answer, in seconds. The default is 300.

    The declaring methods take a block of a declaration in JSON, as `mosaica check --payloads`
    prints it. Declaring a thing again exactly as it stands answers `200` and changes nothing, so a
    declaration is safe to send twice. A different declaration under a name in use is refused,
    except where a method says otherwise.

    The catalogue methods manage who may read and write. A catalogue change ends every session whose
    access it could alter, and its answer counts them in `sessions_ended`; `create_key` answers with
    the new key instead.
    """

    def __init__(self, base: str, credential: str | None = None, timeout: float = 300.0) -> None:
        self.base = base.rstrip("/")
        self.credential = credential
        self.timeout = timeout
        self._limits: dict | None = None

    # ------------------------------------------------------------------ the transport

    def _send(
        self,
        method: str,
        path: str,
        body: bytes | None = None,
        headers: dict | None = None,
        replayable: bool = False,
    ) -> Answer:
        head = request_headers(method, self.credential, headers)
        # An empty body goes as none: urllib labels any body it is given as form data.
        request = urllib.request.Request(
            self.base + path, data=body or None, method=method, headers=head
        )
        started = time.monotonic()
        attempts = 0
        waited = 0.0
        resent = False
        while True:
            attempts += 1
            try:
                with urllib.request.urlopen(request, timeout=self.timeout) as response:
                    text = response.read().decode(errors="replace")
                    return Answer(
                        response.status,
                        _decoded(text),
                        text,
                        attempts,
                        time.monotonic() - started,
                    )
            except urllib.error.HTTPError as refusal:
                text = refusal.read().decode(errors="replace")
                if refusal.code == 429:
                    wait = min(retry_after(429, refusal.headers, _decoded(text)), MAX_BACKOFF)
                    if waited + wait <= RETRY_BUDGET_S:
                        sleep(wait)
                        waited += wait
                        continue
                return Answer(
                    refusal.code, _decoded(text), text, attempts, time.monotonic() - started
                )
            except (urllib.error.URLError, OSError) as unreachable:
                # A request carrying a batch id is sent once more, the same bytes under the same
                # id, which the server answers as a replay where the first arrived; the wait is
                # not taken from a `429`'s budget. Any other, and one unanswered twice, is a
                # refusal the report carries, not an exception out of `commit()`: the pages already
                # acknowledged stay acknowledged, and the report names the one that was not.
                if replayable and not resent:
                    resent = True
                    sleep(retry_after(429, None, None))
                    continue
                return Answer(
                    UNANSWERED,
                    {},
                    f"{self.base + path} did not answer: {unreachable}",
                    attempts,
                    time.monotonic() - started,
                )

    # ------------------------------------------------------------------ the routes

    def status(self) -> dict:
        """Return the server's status report as a dictionary (`GET /control/status`).

        `overlay` counts the deletions and suppressions of items that no compaction has yet removed,
        and `compaction` reports the requests `compact` makes. When the server refuses the request,
        the dictionary is its error body, `{"error": ..., "detail": ...}`, and it is `{}` only when
        no server answered. Call `status_answer()` to read a refusal's status.
        """
        return self.status_answer().body

    def status_answer(self) -> Answer:
        """Return the server's status report as an `Answer` (`GET /control/status`).

        Use it to read the status and detail of a refusal, which `status()` does not give.
        """
        return self._send("GET", "/control/status")

    def limits(self) -> dict:
        """Return the limits on what one request to each write route may carry, so that a client can
        size its requests.

        The limits are the `limits` block of `status()`, read once and kept for the life of this
        `Control`. A request over a limit is refused with `422`.
        """
        if self._limits is None:
            self._limits = self.status().get("limits", {})
        return self._limits

    def ingest(
        self, body: bytes, batch: str, view: str | None = None, strict: bool = False
    ) -> Answer:
        """Create items, edit them or add them to a view, from a batch of rows
        (`POST /control/ingest`).

        - `body`: the rows, as the bytes of an Arrow IPC stream.
        - `batch`: an id for this batch, made fresh for each new one. The server answers the same id
          with the same bytes as a replay that writes nothing twice, which makes a retry safe, and
          refuses the same id with other bytes. When no server answers, the method sends the batch
          once more under its id.
        - `view`: the view the rows are for, which may be left out when the database has one view.
        - `strict`: `True` refuses the whole batch with `409` at its first refused row, so that
          nothing is written.

        A row names its item by its `mosaica_id` and its values of the unique columns. A row that
        names no item creates one at the row's position. A row that names an item edits it, and the
        item keeps its `mosaica_id`; when the item has no row in this view yet, the row adds it
        there. A column left out keeps the value the item stores, and a null clears it.

        A row is refused when it names two items, when its `mosaica_id` names no item, when it names
        no item and carries no coordinates, or when it names an item or sets a unique value that an
        earlier row of the batch did, and the other rows are applied. The answer counts the rows
        `created`, `edited`, `added` and `unchanged`, gives each row's `mosaica_id` in `mosaica_ids`
        with `None` for a refused row, and lists the refused rows in `refused` by position and
        reason. A row whose one change places its item in an annotation changes only the annotation,
        so it is counted unchanged and the membership it adds is counted in `joined`.
        """
        headers = {"content-type": ARROW, "x-mosaica-batch-id": batch}
        if view is not None:
            headers["x-mosaica-view"] = view
        return self._send(
            "POST", "/control/ingest" + _query(strict=strict), body, headers, replayable=True
        )

    def declare_layer(self, payload: dict) -> Answer:
        """Declare an annotation layer, a named set of annotations over items, such as clusters with
        their labels (`PUT /control/layers`).

        The answer carries the layer's `mosaica_id`, with `201` for a new layer and `200` for a
        layer of that name that already holds this declaration. Another declaration under a name in
        use is refused with `422`. The name of a dropped layer may be declared again. When every
        layer space is in use, or the layer declares more levels than one space holds, the
        declaration is refused with `409`.
        """
        return self._send(
            "PUT", "/control/layers", json.dumps(payload).encode(), {"content-type": JSON}
        )

    def declare_view_group(self, name: str, body: dict) -> Answer:
        """Declare a view group, a set of views that share one frame and projection
        (`PUT /control/view_groups/{name}`).

        The group starts with no views, and `create_view` adds them.
        """
        return self._send("PUT", f"/control/view_groups/{_segment(name)}", _json(body),
                          {"content-type": JSON})

    def declare_attribute(self, body: dict) -> Answer:
        """Declare an attribute, a column of values that items carry (`PUT /control/attributes`).

        `body` names the column, and a new column holds no value for any item until a write gives it
        one. `unique` is the one field a declared column may change. Declaring it `True` builds the
        column's index over every stored value, and the answer waits for the build. When two items
        hold the same value, the declaration is refused with `409`.
        """
        return self._send("PUT", "/control/attributes", _json(body), {"content-type": JSON})

    def declare_shards(self, body: dict) -> Answer:
        """Declare the shard size, the number of ids a shard issues to new items before it is sealed
        and the next shard takes them (`PUT /control/shards`).

        `body` is `{"size": n}`. A new size replaces the old one, and a size at or below the open
        shard's high water seals that shard.
        """
        return self._send("PUT", "/control/shards", _json(body), {"content-type": JSON})

    def declare_vocabulary(self, name: str, body: dict) -> Answer:
        """Declare a vocabulary, the set of values a category column takes
        (`PUT /control/vocabularies/{name}`).

        Declaring a vocabulary again as it stands adds the values it sends, as `vocabulary_values`
        does, and changes nothing else.
        """
        return self._send("PUT", f"/control/vocabularies/{_segment(name)}", _json(body),
                          {"content-type": JSON})

    def vocabulary_values(self, name: str, body: dict) -> Answer:
        """Add a page of values to a vocabulary, or change their titles
        (`PATCH /control/vocabularies/{name}/values`).

        `body` holds the page as rows of `{key, title?}`. A key the vocabulary holds keeps its code
        and takes the title sent, and a page removes nothing.
        """
        return self._send(
            "PATCH",
            f"/control/vocabularies/{_segment(name)}/values",
            _json(body),
            {"content-type": JSON},
        )

    def declare_view(self, name: str, body: dict) -> Answer:
        """Declare a view that belongs to no view group (`PUT /control/views/{name}`).

        A view is a coordinate system that items are placed in. This one starts with no rows and
        takes them at its first flush, and it cannot be dropped.
        """
        return self._send("PUT", f"/control/views/{_segment(name)}", _json(body),
                          {"content-type": JSON})

    def create_view(self, group: str, key: str, body: dict) -> Answer:
        """Add a view to a view group (`PUT /control/views/{group}/{key}`).

        - `group`, `key`: the group's name and the new view's key, which together name the view
          `<group>:<key>`.
        - `body`: the view's roster record, which holds its metadata and, where it has one, its
          `visibility`.

        Because the record cannot change once the view exists, sending the record a live view holds
        answers `200` with that view, and another record for its key is refused with `409`. A
        dropped key may be used again.
        """
        return self._send(
            "PUT",
            f"/control/views/{_segment(group)}/{_segment(key)}",
            _json(body),
            {"content-type": JSON},
        )

    def publish(self, layer: str, body: bytes, strict: bool = False) -> Answer:
        """Publish annotations into one level of a layer (`PUT /control/layers/{layer}/artifacts`).

        - `body`: the request's JSON, as bytes, in which each annotation names its members in a
          member table.
        - `strict`: `True` refuses the whole request at its first refused member, with `404` or
          `409`.

        A member that names no item, or two, is left out and listed in the answer's `refused`, and
        the rest of the request is applied as one change. An annotation whose key the level already
        holds is not created again, and the members sent join it. The answer gives each annotation's
        `mosaica_id` in the order sent. A request that would create more annotations than the
        level's layer space has ids left for is refused with `409`.
        """
        return self._send(
            "PUT", _artifacts(layer) + _query(strict=strict), body, {"content-type": JSON}
        )

    def grow(self, layer: str, body: bytes, strict: bool = False) -> Answer:
        """Change the annotations a level already holds (`PATCH /control/layers/{layer}/artifacts`).

        `body` is the request's JSON, as bytes. It can add members, fill parts an annotation lacks,
        and take items out of the set a label's text was written from. Members are refused as for
        `publish`, and `strict` works as it does there. A part that differs from the one the
        annotation holds is refused with `409`, and a key no annotation holds is refused with `422`,
        because this method creates nothing.
        """
        return self._send(
            "PATCH", _artifacts(layer) + _query(strict=strict), body, {"content-type": JSON}
        )

    def changes(self, items: list[dict], strict: bool = False) -> Answer:
        """Delete, suppress or unsuppress items, annotations and layers (`POST /control/changes`).

        - `items`: one record per change, `{"op": ..., "match": {column: value, ...}}`. `op` is
          `"delete"`, `"suppress"` or `"unsuppress"`, and `match` names an item by its `mosaica_id`
          and its values of unique columns, as `Database.addresses` returns them, or an annotation
          or a layer by its `mosaica_id`.
        - `strict`: `True` refuses the whole request at its first change that names no item, with
          `404`, or names two, with `409`.

        A change that names no item, or two, is listed in the answer's `refused`, and the others are
        applied. A deletion or suppression applies to every request that starts after the server
        accepts it. The server never answers this route `429`, so a busy server does not hold a
        deletion back. A request that deletes a layer is refused with `422`, because `drop_layer`
        removes a layer.
        """
        return self._send(
            "POST",
            "/control/changes" + _query(strict=strict),
            json.dumps(items).encode(),
            {"content-type": JSON},
        )

    def drop_layer(self, name: str, wait: bool = False) -> Answer:
        """Remove an annotation layer with all its annotations (`DELETE /control/layers/{name}`).

        Readers stop being served the layer at once. Its name may be declared again afterwards, as a
        new layer that none of the dropped layer's `mosaica_id`s names. `wait` holds the answer
        until the publication carrying the drop is visible, as on `flush`.
        """
        return self._send("DELETE", f"/control/layers/{_segment(name)}" + _wait(wait))

    def drop_view(self, group: str, key: str, wait: bool = False) -> Answer:
        """Remove one view of a view group, with the annotations scoped to it
        (`DELETE /control/views/{group}/{key}`).

        The items the drop leaves in no view are deleted, and the answer's `deleted` counts them.
        `wait` is as for `flush`.
        """
        return self._send(
            "DELETE", f"/control/views/{_segment(group)}/{_segment(key)}" + _wait(wait)
        )

    def compact(self) -> Answer:
        """Ask the server for a compaction, which removes the rows of deleted items from disc
        (`POST /control/compact`).

        The server answers `202` at once with the request's number in `body["request"]`, and then
        compacts every shard with something to reclaim. The outcome is read from `status()`. The
        request has been answered once `compaction.requests.answered` reaches its number. A
        `compaction.requests.last_refusal` whose `request` is that number means a shard was refused
        while answering it, with `detail` saying why and what to do. A higher number means a later
        refusal has replaced the record, and that refusal may or may not concern yours. A request
        answered without a refusal can still have failed, so to confirm that the rows are gone,
        check that `compaction.folds` rose and `overlay.retirable` fell; `compaction.fold_failures`
        counts the compactions that failed or were discarded. When no compaction can start until the
        server restarts, the answer is `503` and nothing is requested.
        """
        return self._send("POST", "/control/compact", b"")

    def flush(self, wait: bool = False) -> Answer:
        """Make the writes sent so far visible to readers sooner, by bringing the next publication
        forward (`POST /control/flush`).

        Writes become visible to readers at a publication, and the answer's `publication` names the
        first one certain to include every request sent before this one.

        - `wait`: `True` holds the answer until readers see that publication, or until the server's
          `serve.visible_wait_max_secs` passes, in which case the answer's `visible` is `false`.
        """
        return self._send("POST", "/control/flush" + _wait(wait), b"")

    # ------------------------------------------------------------------ the catalogue
    #
    # Principals, their passwords and API keys, groups, grants, OIDC providers and sessions. Each
    # route needs `admin`. A change answers `{"sessions_ended": n}`: every live session of every
    # principal, key or provider whose terms or permissions it could change has ended.

    def _json_send(self, method: str, path: str, body: dict) -> Answer:
        return self._send(method, path, _json(body), {"content-type": JSON})

    def list_principals(self) -> Answer:
        """List every local principal (`GET /control/principals`).

        The answer's `principals` holds one record per principal, with its `name`, `kind`,
        `disabled`, `has_password`, and the `terms`, `permissions` and `groups` granted to it
        directly. The answer holds no password or key.
        """
        return self._send("GET", "/control/principals")

    def show_principal(self, name: str) -> Answer:
        """Return one principal's record, as `list_principals` describes it
        (`GET /control/principals/{name}`).

        The answer's status is `404` when no principal has the name.
        """
        return self._send("GET", f"/control/principals/{_segment(name)}")

    def create_principal(self, name: str, kind: str) -> Answer:
        """Create a local principal, a named person or service that can be granted access
        (`POST /control/principals`).

        - `name`: the principal's name. A name already in use is refused with `409`.
        - `kind`: `"person"` or `"service"`.

        The principal starts with no password, key, term or permission.
        """
        return self._json_send("POST", "/control/principals", {"name": name, "kind": kind})

    def change_principal(self, name: str, disabled: bool) -> Answer:
        """Disable or enable a principal (`PATCH /control/principals/{name}`).

        - `disabled`: `True` disables the principal, and `False` enables it again.

        Disabling a principal ends its sessions and the tokens it made for others with `authorise`,
        and the server accepts none of its credentials until it is enabled.
        """
        return self._json_send(
            "PATCH", f"/control/principals/{_segment(name)}", {"disabled": disabled}
        )

    def delete_principal(self, name: str) -> Answer:
        """Delete a principal with its password, API keys, grants and group memberships
        (`DELETE /control/principals/{name}`)."""
        return self._send("DELETE", f"/control/principals/{_segment(name)}")

    def set_password(self, name: str, password: str) -> Answer:
        """Set a principal's password, replacing any it had
        (`PUT /control/principals/{name}/password`).

        A password shorter than the server's `catalogue.min_password_length` is refused with `422`.
        """
        return self._json_send(
            "PUT", f"/control/principals/{_segment(name)}/password", {"password": password}
        )

    def clear_password(self, name: str) -> Answer:
        """Remove a principal's password (`DELETE /control/principals/{name}/password`).

        No password logs the principal in afterwards.
        """
        return self._send("DELETE", f"/control/principals/{_segment(name)}/password")

    def list_keys(self, principal: str) -> Answer:
        """List a principal's API keys (`GET /control/principals/{name}/keys`).

        The answer holds them under `keys`, without their secrets.
        """
        return self._send("GET", f"/control/principals/{_segment(principal)}/keys")

    def create_key(
        self,
        principal: str,
        expires_at: int | None = None,
        permissions: Sequence[str] | None = None,
    ) -> Answer:
        """Issue an API key for a principal (`POST /control/principals/{name}/keys`).

        - `principal`: the principal the key authenticates.
        - `expires_at`: when the key expires, in seconds since 1970, which also ends every session
          made with it. Left out, the key does not expire.
        - `permissions`: the permissions the key is limited to, which never exceed its principal's.
          Left out, the key holds its principal's.

        The answer's `key` is the whole key, which no later answer shows again, so keep it when you
        make it. `prefix` names the key to `list_keys` and `revoke_key`.
        """
        body: dict = {}
        if expires_at is not None:
            body["expires_at"] = int(expires_at)
        if permissions is not None:
            body["permissions"] = list(permissions)
        return self._json_send("POST", f"/control/principals/{_segment(principal)}/keys", body)

    def revoke_key(self, prefix: str) -> Answer:
        """Revoke an API key by its prefix (`DELETE /control/keys/{prefix}`).

        Revoking a key ends every session made with it, including the tokens `authorise` made with
        it.
        """
        return self._send("DELETE", f"/control/keys/{_segment(prefix)}")

    def list_groups(self) -> Answer:
        """List every local group, with its terms, permissions and members, under `groups` in the
        answer (`GET /control/groups`)."""
        return self._send("GET", "/control/groups")

    def show_group(self, name: str) -> Answer:
        """Return one group's record (`GET /control/groups/{name}`).

        The answer's status is `404` when no group has the name.
        """
        return self._send("GET", f"/control/groups/{_segment(name)}")

    def create_group(self, name: str) -> Answer:
        """Create a local group, whose members hold every term and permission granted to it
        (`POST /control/groups`).

        A name already in use is refused with `409`.
        """
        return self._json_send("POST", "/control/groups", {"name": name})

    def delete_group(self, name: str) -> Answer:
        """Delete a group with its grants and memberships (`DELETE /control/groups/{name}`)."""
        return self._send("DELETE", f"/control/groups/{_segment(name)}")

    def add_member(self, group: str, principal: str) -> Answer:
        """Add a principal to a group (`PUT /control/groups/{name}/members/{principal}`)."""
        return self._send("PUT", f"/control/groups/{_segment(group)}/members/{_segment(principal)}")

    def remove_member(self, group: str, principal: str) -> Answer:
        """Remove a principal from a group (`DELETE /control/groups/{name}/members/{principal}`)."""
        return self._send(
            "DELETE", f"/control/groups/{_segment(group)}/members/{_segment(principal)}"
        )

    def grant(
        self,
        principal: str | None = None,
        group: str | None = None,
        term: str | None = None,
        terms: Sequence[str] | None = None,
        permission: str | None = None,
    ) -> Answer:
        """Grant access terms or a permission to a principal or a group (`POST /control/grants`).

        - `principal`, `group`: the grantee.
        - `term`, `terms`: an access term, which says what the grantee may see, or several, which
          are granted as one change and refused together if one is refused.
        - `permission`: what the grantee may do, one of `"read"`, `"write"`, `"authorise-as"`,
          `"admin"`, `"read-all"` and `"write-all"`.

        Name exactly one grantee and exactly one of `term`, `terms` and `permission`. The server
        refuses any other combination with `422`. A grant already held changes nothing.
        """
        return self._json_send("POST", "/control/grants", _grant(principal, group, term, terms, permission))

    def revoke_grant(
        self,
        principal: str | None = None,
        group: str | None = None,
        term: str | None = None,
        terms: Sequence[str] | None = None,
        permission: str | None = None,
    ) -> Answer:
        """Take back terms or a permission that `grant` gave, with the same arguments
        (`POST /control/grants/revoke`).

        A grant that is not held changes nothing.
        """
        return self._json_send(
            "POST", "/control/grants/revoke", _grant(principal, group, term, terms, permission)
        )

    def list_providers(self) -> Answer:
        """List every OIDC provider, whether declared through this API or in `mosaica.toml`, under
        `providers` in the answer (`GET /control/providers`)."""
        return self._send("GET", "/control/providers")

    def show_provider(self, name: str) -> Answer:
        """Return one OIDC provider's record (`GET /control/providers/{name}`).

        The answer's status is `404` when no provider has the name.
        """
        return self._send("GET", f"/control/providers/{_segment(name)}")

    def put_provider(
        self,
        name: str,
        issuer: str,
        audience: str,
        jwks_url: str,
        claim_rules: Sequence[dict] | None = None,
        role_mappings: Sequence[dict] | None = None,
    ) -> Answer:
        """Declare an OIDC provider whose access tokens the database accepts, or replace one whole
        (`PUT /control/providers/{name}`).

        - `issuer`, `audience`: the `iss` the provider's tokens carry and the `aud` they must hold.
        - `jwks_url`: where the provider publishes its signing keys, over `https`, or over `http` to
          a loopback address.
        - `claim_rules`: rules that turn a claim's values into access terms, each
          `{"claim": ..., "template": ...}`, such as
          `{"claim": "groups[*]", "template": "{value}"}`.
        - `role_mappings`: rules that give an identity a local group's terms and permissions when
          one of its claims holds a value exactly, each
          `{"claim": ..., "value": ..., "group": ...}`.

        Replacing a provider ends every session authorised through it. A provider declared in
        `mosaica.toml` is refused with `409`, because it is changed there.
        """
        body: dict = {"issuer": issuer, "audience": audience, "jwks_url": jwks_url}
        if claim_rules is not None:
            body["claim_rules"] = list(claim_rules)
        if role_mappings is not None:
            body["role_mappings"] = list(role_mappings)
        return self._json_send("PUT", f"/control/providers/{_segment(name)}", body)

    def drop_provider(self, name: str) -> Answer:
        """Remove an OIDC provider, which ends every session authorised through it
        (`DELETE /control/providers/{name}`).

        A provider declared in `mosaica.toml` is refused with `409`, because it is removed there.
        """
        return self._send("DELETE", f"/control/providers/{_segment(name)}")

    def list_sessions(self, principal: str | None = None, provider: str | None = None) -> Answer:
        """List the live sessions under `sessions` in the answer (`GET /control/sessions`).

        - `principal`: lists only a local principal's sessions, including those `authorise` made for
          it.
        - `provider`: lists only the sessions authorised through an OIDC provider.

        Name at most one of them, and leave both out to list every session.
        """
        query = {k: v for k, v in (("principal", principal), ("provider", provider)) if v is not None}
        path = "/control/sessions"
        if query:
            path += "?" + urllib.parse.urlencode(query)
        return self._send("GET", path)

    def end_sessions(
        self,
        token_id: int | None = None,
        principal: str | None = None,
        provider: str | None = None,
    ) -> Answer:
        """End one session, or every session of a local principal or an OIDC provider
        (`POST /control/sessions/end`).

        Name exactly one of `token_id`, `principal` and `provider`. A session's `token_id` is in
        `list_sessions` and on the `Token` that `authorise` made, and an ended session's token stops
        working.
        """
        body = {
            k: v
            for k, v in (("token_id", token_id), ("principal", principal), ("provider", provider))
            if v is not None
        }
        return self._json_send("POST", "/control/sessions/end", body)


def _grant(principal, group, term, terms, permission) -> dict:
    """A grant's body: the arguments given, under the route's own names."""
    named = {
        "principal": principal,
        "group": group,
        "term": term,
        "terms": None if terms is None else listed(terms),
        "permission": permission,
    }
    return {k: v for k, v in named.items() if v is not None}


def _wait(wait: bool) -> str:
    """`?wait=visible`, where the caller asked for it.

    The route holds its answer until the publication its acknowledgement names has happened, and
    pulls the tick forward to get there. One request of a commit carries it, the closing flush: a
    waited request asks for a tick of its own, so a commit that waited on every page would publish
    per page.
    """
    return "?wait=visible" if wait else ""


def _query(strict: bool = False) -> str:
    """`?strict=true`, where the caller asked for it; the routes default to `false`."""
    return "?strict=true" if strict else ""


def _json(body: dict) -> bytes:
    return json.dumps(body).encode()


def _segment(name: str) -> str:
    """One path segment, percent-encoded: a name the router must see whole."""
    return urllib.parse.quote(name, safe="")


def _artifacts(layer: str) -> str:
    """The artifacts route of one layer, its name percent-encoded.

    A layer name is path-shaped here (`clusters/kmeans`) and the route matches one segment, so an
    unencoded name is a 404 at the router rather than a refusal from the handler.
    """
    return f"/control/layers/{urllib.parse.quote(layer, safe='')}/artifacts"


def _decoded(text: str) -> dict:
    try:
        value = json.loads(text)
    except ValueError:
        return {}
    return value if isinstance(value, dict) else {"value": value}


def arrow_body(table: Any) -> bytes:
    """One Arrow IPC stream carrying a table, which is what a row route takes."""
    import pyarrow as pa
    import pyarrow.ipc as ipc

    batch = table.combine_chunks().to_batches()
    schema = table.schema
    sink = pa.BufferOutputStream()
    with ipc.new_stream(sink, schema) as writer:
        for one in batch:
            writer.write_batch(one)
    return sink.getvalue().to_pybytes()
