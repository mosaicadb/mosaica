# Operate a deployment

This guide covers running Mosaica once it is installed, [under systemd](systemd.md) or [with Docker](docker.md). Before anyone else uses the server, decide who may reach each of its three addresses and how much memory it gets. After that, the server mostly looks after itself: you check that it's healthy, back it up, and restart it when you change a setting. Most of what it does on its own is compaction, which has [a page of its own](compaction.md), as do [sizing and tuning](sizing.md), [putting it behind TLS](tls.md) and [rebuilding the bundle](rebuild.md).

The examples use the systemd install, run as the `mosaica` user in `/srv/mosaica`, whose control address is a Unix socket. Under Docker, run them from `~/mosaica-docker`, use ports 9161 and 9162 in place of 9151 and 9152, and write `--control http://127.0.0.1:9163` in place of `--control unix:/run/mosaica/control.sock`. The commands that use the control address read the operator credential from `MOSAICA_CREDENTIAL`:

```console
mosaica$ export MOSAICA_CREDENTIAL=$(cat secrets/operator.secret)
```

## Addresses and credentials

| Address | Routes | Who should reach it |
|---|---|---|
| Viewer | `/v1/*`, `/healthz`, `/readyz` | Browsers, each with its own token, through a TLS proxy. People who log in with a password or an API key do it here, at `/v1/login` |
| Session | `/session/authorise`, `/session/revoke`, `/healthz`, `/readyz` | Your application's backend, which signs people in and asks Mosaica for their tokens with an API key whose user holds `authorise-as` |
| Control | `/control/*` | You, and whatever loads data into the database |

Keep the session and control addresses off the internet. An API key whose user holds `authorise-as` can start a session for any user, and so read whatever any user's attributes admit. The operator credential holds every permission: it can start a session that sees everything, change the catalogue, and delete your data or add to it. The viewer address serves only what each token allows, and it's the one browsers need. It serves plain HTTP and accepts passwords, so put it behind TLS, as [Put Mosaica behind TLS](tls.md) describes.

Both installs keep the operator credential in a file called `operator.secret`: under systemd in `/srv/mosaica/secrets`, and under Docker in `~/mosaica-docker/secrets`, which Compose mounts into the container as `/run/secrets`. The server reads the file once, when it starts, so to change the credential, replace the file and restart. Users, groups, API keys and grants live in the catalogue, the directory `[catalogue] dir` names, and change without a restart. [Users and roles](../system/users-and-roles.md) covers them.

??? info "The credential in an environment variable"
    Instead of a file, `operator_credential_env` can name an environment variable. Anything else running as the same user can read a process's environment from `/proc`, so a file is the better choice. Either way, the server trims the whitespace around the credential, and refuses to start without one, even if nothing ever calls the control address:

    ```text
    mosaica serve: refused to start: there is no operator credential; set `operator_credential_file` or `operator_credential_env` under [serve] and put the secret in that file or variable
    ```

A viewer's session lasts at most `token_max_lifetime` seconds, set under `[disclosure]`, and a change in the catalogue that affects a user ends their sessions, so their next request starts a new one. [When a session ends](../system/access-control.md#when-a-session-ends) lists every cause.

## Memory

Set a memory cap on the server (`MemoryMax` under systemd, or the container's memory limit) above the limits on its caches, which come to about 7.7 GiB by default. Memory beyond that keeps more of the bundle in memory and makes the server faster. [Size and tune a deployment](sizing.md#memory) explains how to choose the cap and when to change the cache limits.

## Health and readiness

The viewer and session addresses both answer `/healthz` and `/readyz`, with no credential. Send traffic by `/readyz`: it answers 200 while the server is taking writes normally, and 503 when it isn't, or when the server has had to open an older state of the bundle because the newest one on disc failed its checks. `/healthz` answers 200 whenever the process answers at all. The control address has neither.

`mosaica health` makes the same check from the command line. It reads `mosaica.toml`, asks the viewer address for `/readyz`, and exits 0 on a 200. The Docker image uses it as its health check.

```console
mosaica$ mosaica health && echo ready
ready
```

When nothing is listening it says so and exits 1:

```text
mosaica health: no server answering at 127.0.0.1:9151: Connection refused (os error 111)
```

From outside the machine, check `/readyz` through the [TLS proxy](tls.md), whose example configuration passes it through, or against the session address from inside your network.

For more than up or down, `mosaica status` prints the server's status report: how far writes have got, queue depths, compaction, memory and the limits on each write. `write_executor.posture` is the state of the write side, one of `not-started`, `running`, `wal-poisoned` or `dead`, and only `running` is ready.

A disc that hangs on a write, rather than failing it, leaves the posture at `running` and `/readyz` at 200 while nothing is written. [Health and readiness](../system/serving.md#health-and-readiness) covers what the probe can and cannot see.

## What the server does on its own

New and edited items become visible at each **flush**, every 90 seconds by default, or sooner once 40,000 rows are waiting. Each flush adds a segment to the bundle, and the server merges small segments as they build up.

Deleted and edited items stay in the bundle, hidden, until a **compaction** removes them. The server compacts on its own, mostly in a four-hour window from midnight UTC. Set `compaction_window_start` under `[ingest]` to the start of your quietest hours, since requests straight after a compaction are slower while the server rebuilds its caches. A compaction needs free disc space of one and a half times the bundle. [Schedule compactions](compaction.md) covers the rest.

## What the server keeps on disc

`[bundle]` in `mosaica.toml` names three places:

| Key | What it holds | If it is lost |
|---|---|---|
| `path` | The bundle: every item, its position, its fields, the search indexes and the identity key. The server writes to it as it runs | The database. Restore it from a backup |
| `wal` | The write-ahead log: every change since the last flush, in files named after this path, such as `wal-000001.log` | Every change acknowledged since the last flush |
| `cache` | Work the server can redo, such as each set of access attributes' visible items and their annotation counts | Nothing but time. The server rebuilds what it needs |

Back up the bundle and the log together, with the server stopped. A copy of one without the other is the database as it stood at some other moment. [Disc](sizing.md#disc) explains how much space each takes.

Keep the bundle and the log on a local disc. The server locks the bundle's directory so that a second server can't write to it, and that lock doesn't work reliably over NFS or SMB, where two servers writing one bundle overwrite each other's changes.

Anyone who can read the bundle's files can read every item in it, whatever its access label, and the identity key too. Access control applies to what the server sends, not to the files.

## Stopping and restarting

SIGTERM or SIGINT stops the server at once, without waiting for requests in progress, and it writes `mosaica serve: stopped on SIGTERM` as it goes. A browser partway through a response gets it cut short.

On starting, the server opens the newest state of the bundle on disc that passes its checks and replays the log over it, which brings back every change it acknowledged before it stopped. Sessions are held only in memory, so after a restart each browser asks your backend for a new token. [Durability and restarts](../system/write-path.md#durability-and-restarts) has the detail.

??? info "Stopping when standard input closes"
    With `--exit-on-stdin-close`, the server also stops, as SIGTERM stops it, when its standard input reaches its end. A process that starts the server and holds a pipe to its standard input can then stop it by closing the pipe, and the server stops too when that process ends in any way. Standard input that is empty or `/dev/null` stops the server as soon as it starts, so leave the flag off under systemd or Docker.

## When a write to the log fails

The server writes every change to its write-ahead log, and forces it to disc, before it tells the caller the change succeeded, so a change it has acknowledged survives the server being stopped or killed. When a write to the log fails, the server tells you in one of two ways:

- If appending the record fails, the posture becomes `wal-poisoned` and `/readyz` answers 503 until the server restarts.
- If the record was appended but forcing it to disc fails, a deletion or suppression is tried again, twice, and ingest isn't. When the write still fails, the server throws away what didn't reach the disc and goes back to `running`, and `/readyz` answers 200 again. The signs are `write_executor.wal_recoveries` going up by one in `/control/status`, and a line in the log beginning `ALARM` and ending "Restart this node." The server isn't back to normal: it makes no new items visible, never trims the log and starts no compaction.

Either way, a deletion or suppression whose write failed is still applied in memory, so the item stays hidden. The caller is answered 500, with a detail that includes "a change in it may be in force, so do not treat this as a no-op". An unsuppress whose write failed isn't applied. Nothing on disc records the deletions and suppressions that were applied, so a restart forgets them.

!!! warning
    A restart can bring a deleted or suppressed item back into view if the change that hid it was answered 500. After a failed write:

    1. Have whatever sends deletions and suppressions send again every change that was answered 500. If the log has recovered, this writes them to disc.
    2. Fix the disc.
    3. Restart the server.
    4. Send every change that was answered 500 once more. This catches any that failed again in step 1.

    Restart the server when its process exits, and never because `/readyz` answers 503. The systemd unit and the Compose file in these guides both work that way.
