# Architecture Decision Records (ADRs)

This directory contains architecture decisions for ThunderForgeVTT, captured using the Nygard ADR template format.

## Index

| Date                                                                             | Status   | Title                                                                                         |
| -------------------------------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------- |
| [20260501-000](./20260501-000-durable_objects_with_graphql_event_driven_sync.md) | Accepted | Durable Objects via GraphQL Event-Driven Synchronization Architecture                         |
| [20260504-000](./20260504-000-fantasy_ui_shell_with_radix_and_wrapped_tldraw.md) | Accepted | Fantasy UI Shell with Radix Primitives, Dicebear Identity Surfaces, and Wrapped tldraw Chrome |
| [20260504-001](./20260504-001-rest_auth_with_db_backed_cookie_sessions.md)       | Accepted | Session Cookie Strategy for Unified Authentication                                            |
| [20260504-002](./20260504-002-unified_authentication_model.md)                   | Accepted | Unified Authentication Model (Local + OAuth)                                                  |
| [20260504-003](./20260504-003-user_data_ownership_model.md)                      | Accepted | User Data Ownership Model                                                                     |
| [20260504-004](./20260504-004-user_data_export_contract.md)                      | Accepted | User Data Export Contract                                                                     |
| [20260504-005](./20260504-005-user_permanent_deletion_contract.md)               | Accepted | User Permanent Deletion Contract                                                              |
| [20260504-006](./20260504-006-oauth_linking_safety_rules.md)                     | Accepted | OAuth Linking Safety Rules                                                                    |
| [20260504-007](./20260504-007-no_auto_provisioning_policy.md)                    | Accepted | No Auto-Provisioning Policy                                                                   |
| [20260504-008](./20260504-008-bootstrap_admin_exception.md)                      | Accepted | Bootstrap Admin Exception                                                                     |
| [20260504-009](./20260504-009-created_by_updated_by_enforcement.md)              | Accepted | Created-By / Updated-By Enforcement Across All Tables                                         |
| [20260504-010](./20260504-010-ownership_fields_on_persisted_tables.md)           | Accepted | Ownership Fields on Persisted Tables                                                          |
| [20260504-011](./20260504-011-export_my_data_contract.md)                        | Accepted | Export-My-Data Contract                                                                       |
| [20260504-012](./20260504-012-delete_my_data_contract.md)                        | Accepted | Delete-My-Data Contract                                                                       |
| [20260504-013](./20260504-013-graphql_ownership_enforcement.md)                  | Accepted | GraphQL Ownership Enforcement                                                                 |
| [20260504-014](./20260504-014-placeholder_domain_objects.md)                     | Accepted | Placeholder Domain Objects in the API Contract                                                |
| [20260504-015](./20260504-015-admin_settings_page_architecture.md)               | Accepted | Admin Settings Page Architecture                                                              |
| [20260504-016](./20260504-016-analytics_data_sources.md)                         | Accepted | Analytics Data Sources                                                                        |
| [20260504-017](./20260504-017-oauth_provider_configuration_contract.md)          | Accepted | OAuth Provider Configuration Contract                                                         |
| [20260504-018](./20260504-018-manifest_editing_policy.md)                        | Accepted | Manifest Editing Policy                                                                       |
| [20260504-019](./20260504-019-disk_usage_calculation_strategy.md)                | Accepted | Disk Usage Calculation Strategy                                                               |
| [20260504-020](./20260504-020-world_creation_contract.md)                        | Accepted | World Creation Contract                                                                       |
| [20260504-021](./20260504-021-world_metadata_schema.md)                          | Accepted | World Metadata Schema                                                                         |
| [20260504-022](./20260504-022-world_routing_rules.md)                            | Accepted | World Routing Rules                                                                           |
| [20260504-023](./20260504-023-world_ownership_rules.md)                          | Accepted | World Ownership Rules                                                                         |
| [20260504-024](./20260504-024-world_placeholder_domain_objects.md)               | Accepted | World Placeholder Domain Objects                                                              |

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
