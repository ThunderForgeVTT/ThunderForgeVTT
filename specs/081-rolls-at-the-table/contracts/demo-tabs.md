# Contract: the demo's tab messages

Channel: `BroadcastChannel("thunderforge-demo")`. Lock:
`navigator.locks` name `"thunderforge-demo-world"`.

| Message                                             | From   | Meaning                                     |
| --------------------------------------------------- | ------ | ------------------------------------------- |
| `{ kind: "request", id, viewer, query, variables }` | guest  | run this operation as `viewer`              |
| `{ kind: "answer", id, result }`                    | holder | the `ExecutionResult` for request `id`      |
| `{ kind: "event", event }`                          | holder | a released world event, unfiltered          |
| `{ kind: "holder" }`                                | holder | a new holder took over; resend what is open |

- `id` is a random UUID per request; an answer for an unknown id is
  ignored.
- Every tab filters `event` for its own viewer before delivering it.
- Uploads (`File` values) are structured-cloned by the channel.
