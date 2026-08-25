# Contract: `crucible-server` HTTP surface

This is the wire contract `RemoteAdjudicator` (the main server's HTTP client
implementation of `SessionAdjudicator`) speaks to a running `crucible-server`
process. It exists purely so `RemoteAdjudicator` and the in-process
`LocalAdjudicator` are interchangeable — no other consumer is expected.

## `POST /adjudicate`

Request body — `AdjudicationRequest` (data-model.md), JSON:

```json
{
  "world_id": "<uuid>",
  "actor_id": "<uuid>",
  "kind": "Move",
  "payload": { "...": "action-specific detail" }
}
```

Response body — `AdjudicationResult` (data-model.md), JSON, `200 OK`:

```json
{
  "outcome": "Accepted",
  "payload": null,
  "reason": null
}
```

Error responses:

- `400 Bad Request` — the request body failed to deserialize into
  `AdjudicationRequest`, or its `kind` is unrecognized. `RemoteAdjudicator`
  maps this to `SessionAdjudicatorError::InvalidRequest`.
- Connection failure, timeout, or any non-2xx/400 status —
  `RemoteAdjudicator` maps this to
  `SessionAdjudicatorError::RemoteUnavailable` (research.md §3) rather than
  attempting to interpret arbitrary server errors as adjudication outcomes.

## `GET /health`

Trivial liveness check, `200 OK` with an empty body when the process is up
and able to serve `/adjudicate`. Not used by `RemoteAdjudicator` itself in
this spec's scope (no retry/circuit-breaker logic here — research.md §3) —
included because it's the minimum a future orchestration layer (explicitly
out of scope, ADR-047) would need, and costs nothing to add now alongside
the router this spec is already building.

## Explicitly not in this contract

- No authentication/authorization on this surface. `crucible-server` is not
  intended to be reachable from anywhere except the main `thunderforge`
  server in this spec's scope (both processes co-located, e.g. same
  container network) — access control at that network boundary is a
  deployment/operations concern, not this crate's. Revisit if/when a future
  multi-tenant orchestration layer changes that trust boundary.
- No versioning scheme on the request/response shapes beyond "both sides of
  this contract are built from the same crate version" — acceptable for a
  same-repo, same-release-cadence internal contract; revisit if
  `crucible-server` and the main server are ever deployed at independently
  version-pinned points.
