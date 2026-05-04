# Architecture Decision Records (ADRs)

This directory contains architecture decisions for ThunderForgeVTT, captured using the Nygard ADR template format.

## Index

| Date                                                                             | Status   | Title                                                                                         |
| -------------------------------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------- |
| [20260501-000](./20260501-000-durable_objects_with_graphql_event_driven_sync.md) | Accepted | Durable Objects via GraphQL Event-Driven Synchronization Architecture                         |
| [20260504-000](./20260504-000-fantasy_ui_shell_with_radix_and_wrapped_tldraw.md) | Accepted | Fantasy UI Shell with Radix Primitives, Dicebear Identity Surfaces, and Wrapped tldraw Chrome |
| [20260504-001](./20260504-001-rest_auth_with_db_backed_cookie_sessions.md)       | Accepted | REST Authentication with Database-Backed Cookie Sessions                                      |

## Guidelines

- **Filename Format**: `YYYYMMDD-NNN-descriptive_name_with_underscores.md`

  - `YYYYMMDD`: Decision date (20260501)
  - `NNN`: Iteration number (000, 001, 002, etc. if multiple on same day)
  - `descriptive_name`: Snake-case, concise summary

- **Template**: Use Nygard format (Status, Context, Decision, Rationale/Y-Statement, Consequences)

- **Conventions**:
  - Keep decisions focused on a single architectural concern
  - Include diagrams using Mermaid or ASCII art for clarity
  - Reference related decisions when applicable
  - Use Y-Statement format for decision rationale (see ADR-000 as example)

## How to Add a New ADR

1. Create file with next sequential number: `YYYYMMDD-NNN-topic_name.md`
2. Use the Nygard template from ADR-000 as reference
3. Update this index with new entry
4. Commit with message: `docs: add ADR-NNN for {topic}`

## References

- [adr.github.io](https://adr.github.io/) - ADR standards and templates
- [Documenting Architecture Decisions](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions) - Michael Nygard's original blog post
- [Y-Statements](https://medium.com/@docsoc/y-statements-10eb07b5a177) - Enhanced format used here
