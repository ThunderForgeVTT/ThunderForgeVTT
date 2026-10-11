# Running your own instance

`compose.yml` is the whole install: the ThunderForge server, its PostgreSQL
database, RustFS object storage for uploads, and Mailpit, which catches the
mail the instance sends. This page covers what it puts on your machine's
network and how to reach the parts it keeps to itself.

## What is published

Only the app. After `docker compose up -d --build` the one address is
**http://localhost:42080** (`THUNDERFORGE_PORT` moves it; move
`THUNDERFORGE_PUBLIC_URL` with it).

The database, storage and Mailpit are reachable by the app inside the
stack's own network and from nowhere else. Nothing about running the
instance needs them on the host.

## Reaching the backing services

For a one-off command, go through compose. No container has a fixed name, so
use the service name:

```sh
docker compose exec postgres psql -U postgres thunderforge
docker compose exec postgres pg_dump -U postgres thunderforge > backup.sql
```

When you want them on the host, add `compose.ports.yml`:

```sh
docker compose -f compose.yml -f compose.ports.yml up -d
```

| Service    | Address                | What for                                            |
| ---------- | ---------------------- | --------------------------------------------------- |
| Mailpit    | http://127.0.0.1:42825 | every mail the instance sends, delivered nowhere    |
| Mailpit    | 127.0.0.1:42025        | its SMTP port                                       |
| PostgreSQL | 127.0.0.1:42432        | the database (user `postgres`, password `password`) |
| RustFS     | 127.0.0.1:42900        | object storage for uploaded images                  |

Every one binds `127.0.0.1`, so none is reachable from another machine.
`THUNDERFORGE_POSTGRES_PORT`, `THUNDERFORGE_RUSTFS_PORT`,
`THUNDERFORGE_MAILPIT_SMTP_PORT` and `THUNDERFORGE_MAILPIT_UI_PORT` move
them. Run the same command without `-f compose.ports.yml` to take them away
again.

## Mail while you try it out

The first-run wizard asks how the instance sends mail. For a local try,
answer with host `mailpit`, port `1025`, no security, then add
`compose.ports.yml` to read what arrives at http://127.0.0.1:42825. A real
table points the same step at a real mail relay.

## Upgrading from an earlier install

Before this change `compose.yml` published PostgreSQL on 42432, RustFS on
42900 and Mailpit on 42025 and 42825, on every interface, and named its
containers `thunderforge-app-*`. If a script of yours used those ports, add
`compose.ports.yml`: the numbers are the same, now on `127.0.0.1`. If it used
`docker exec thunderforge-app-postgres`, use
`docker compose exec postgres` instead. Your data is untouched; the volumes
keep their names.
