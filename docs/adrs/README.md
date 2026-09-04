# Architecture Decision Records (ADRs)

This directory contains architecture decisions for ThunderForgeVTT, captured using the Nygard ADR template format.

## Index

| Date                                                                                  | Status   | Title                                                                                         |
| ------------------------------------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------- |
| [20260501-000](./20260501-000-durable_objects_with_graphql_event_driven_sync.md)      | Accepted | Durable Objects via GraphQL Event-Driven Synchronization Architecture                         |
| [20260504-000](./20260504-000-fantasy_ui_shell_with_radix_and_wrapped_tldraw.md)      | Accepted | Fantasy UI Shell with Radix Primitives, Dicebear Identity Surfaces, and Wrapped tldraw Chrome |
| [20260504-001](./20260504-001-rest_auth_with_db_backed_cookie_sessions.md)            | Accepted | Session Cookie Strategy for Unified Authentication                                            |
| [20260504-002](./20260504-002-unified_authentication_model.md)                        | Accepted | Unified Authentication Model (Local + OAuth)                                                  |
| [20260504-003](./20260504-003-user_data_ownership_model.md)                           | Accepted | User Data Ownership Model                                                                     |
| [20260504-004](./20260504-004-user_data_export_contract.md)                           | Accepted | User Data Export Contract                                                                     |
| [20260504-005](./20260504-005-user_permanent_deletion_contract.md)                    | Accepted | User Permanent Deletion Contract                                                              |
| [20260504-006](./20260504-006-oauth_linking_safety_rules.md)                          | Accepted | OAuth Linking Safety Rules                                                                    |
| [20260504-007](./20260504-007-no_auto_provisioning_policy.md)                         | Accepted | No Auto-Provisioning Policy                                                                   |
| [20260504-008](./20260504-008-bootstrap_admin_exception.md)                           | Accepted | Bootstrap Admin Exception                                                                     |
| [20260504-009](./20260504-009-created_by_updated_by_enforcement.md)                   | Accepted | Created-By / Updated-By Enforcement Across All Tables                                         |
| [20260504-010](./20260504-010-ownership_fields_on_persisted_tables.md)                | Accepted | Ownership Fields on Persisted Tables                                                          |
| [20260504-011](./20260504-011-export_my_data_contract.md)                             | Accepted | Export-My-Data Contract                                                                       |
| [20260504-012](./20260504-012-delete_my_data_contract.md)                             | Accepted | Delete-My-Data Contract                                                                       |
| [20260504-013](./20260504-013-graphql_ownership_enforcement.md)                       | Accepted | GraphQL Ownership Enforcement                                                                 |
| [20260504-014](./20260504-014-placeholder_domain_objects.md)                          | Accepted | Placeholder Domain Objects in the API Contract                                                |
| [20260504-015](./20260504-015-admin_settings_page_architecture.md)                    | Accepted | Admin Settings Page Architecture                                                              |
| [20260504-016](./20260504-016-analytics_data_sources.md)                              | Accepted | Analytics Data Sources                                                                        |
| [20260504-017](./20260504-017-oauth_provider_configuration_contract.md)               | Accepted | OAuth Provider Configuration Contract                                                         |
| [20260504-018](./20260504-018-manifest_editing_policy.md)                             | Accepted | Manifest Editing Policy                                                                       |
| [20260504-019](./20260504-019-disk_usage_calculation_strategy.md)                     | Accepted | Disk Usage Calculation Strategy                                                               |
| [20260504-020](./20260504-020-world_creation_contract.md)                             | Accepted | World Creation Contract                                                                       |
| [20260504-021](./20260504-021-world_metadata_schema.md)                               | Accepted | World Metadata Schema                                                                         |
| [20260504-022](./20260504-022-world_routing_rules.md)                                 | Accepted | World Routing Rules                                                                           |
| [20260504-023](./20260504-023-world_ownership_rules.md)                               | Accepted | World Ownership Rules                                                                         |
| [20260504-024](./20260504-024-world_placeholder_domain_objects.md)                    | Accepted | World Placeholder Domain Objects                                                              |
| [20260504-025](./20260504-025-pack_crate_naming_convention.md)                        | Stub     | Pack Crate Naming Convention _(file is empty — content not yet written)_                      |
| [20260504-026](./20260504-026-pack_architecture_and_pack_type_standard.md)            | Stub     | Pack Architecture and Pack Type Standard _(file is empty — content not yet written)_          |
| [20260504-027](./20260504-027-game_system_packaging_and_manifest_contract.md)         | Stub     | Game System Packaging and Manifest Contract _(file is empty — content not yet written)_       |
| [20260504-028](./20260504-028-game_systems_db_model_and_ownership_rules.md)           | Accepted | Game Systems DB Model and Ownership Rules                                                     |
| [20260504-029](./20260504-029-runtime_module_loading_and_security.md)                 | Accepted | Runtime Module Loading and Security                                                           |
| [20260504-030](./20260504-030-compendium_pack_format.md)                              | Stub     | Compendium Pack Format _(file is empty — content not yet written)_                            |
| [20260505-031](./20260505-031-scene_domain_model.md)                                  | Accepted | Scene Domain Model                                                                            |
| [20260505-032](./20260505-032-canvas_rendering_strategy_bevy.md)                      | Accepted | Canvas Rendering Strategy (Bevy)                                                              |
| [20260505-033](./20260505-033-token_data_model_and_ownership.md)                      | Accepted | Token Data Model & Ownership                                                                  |
| [20260505-034](./20260505-034-fog_of_war_implementation.md)                           | Accepted | Fog of War Implementation                                                                     |
| [20260505-035](./20260505-035-player_view_architecture.md)                            | Accepted | Player View Architecture                                                                      |
| [20260505-036](./20260505-036-extensible_system_agnostic_actor_data_architecture.md)  | Accepted | Extensible System-Agnostic Actor Data Architecture (Type-Indexed JSONB)                       |
| [20260820-037](./20260820-037-native_canvas_authoring_supersedes_tldraw.md)           | Accepted | Native Bevy Canvas Authoring Supersedes Wrapped tldraw                                        |
| [20260820-038](./20260820-038-canvas_core_crate_split_for_native_testability.md)      | Accepted | Split Canvas-Authoring Logic into a Native-Testable Core Crate                                |
| [20260820-039](./20260820-039-rustfs_scoped_asset_storage.md)                         | Accepted | RustFS Scoped Asset Storage                                                                   |
| [20260821-040](./20260821-040-unify_token_backing_store.md)                           | Accepted | Unify Token Backing Store onto the Scene-Scoped `tokens` Table                                |
| [20260821-041](./20260821-041-env_var_oauth_provider_configuration.md)                | Accepted | Environment-Variable OAuth Provider Configuration, Layered on the Existing Admin-Panel Model  |
| [20260821-042](./20260821-042-oauth_auto_provisioning_on_first_login.md)              | Accepted | OAuth Auto-Provisioning on First Login                                                        |
| [20260823-043](./20260823-043-content_moderation_and_dmca_safe_harbor.md)             | Accepted | Content Moderation and DMCA Safe-Harbor Enforcement Boundary                                  |
| [20260823-044](./20260823-044-dice_rolling_engine_shared_crate_and_trust_boundary.md) | Accepted | Dice Rolling Engine — Shared Crate and Server-Authoritative Trust Boundary                    |
| [20260823-045](./20260823-045-genie_session_state_two_party_consent.md)               | Accepted | Genie Session State — Two-Party-Consent Authorization for Session Resource Trades             |
| [20260824-046](./20260824-046-server_authoritative_active_scene.md)                   | Accepted | Server-Authoritative Active Scene, Broadcast Over the Existing World-Events Transport         |
| [20260825-047](./20260825-047-crucible_session_adjudication_crate.md)                 | Accepted | Crucible — A Pluggable, Dual-Mode Session-Adjudication Crate                                  |
| [20260825-048](./20260825-048-graphql_subscription_client_transport.md)               | Accepted | `graphql-ws` as the Client-Side Live-Sync Transport (Recorded Post-Hoc)                       |
| [20260825-049](./20260825-049-share_link_dmca_repository_determination.md)            | Accepted | Share Links Are Not a Centralized Public Repository (DMCA Guardrail Determination)            |
| [20260826-050](./20260826-050-permission_declaration_and_world_access_links.md)       | Accepted | One Permission Declaration, and World Invites as Revocable Access Links                       |
| [20260826-051](./20260826-051-no_ai_game_master.md)                                   | Accepted | **ThunderForgeVTT Will Never Build an AI Game Master**                                        |
| [20260826-052](./20260826-052-client-cache-offline-and-peer-adjudication.md)          | Accepted | The Client May Hold, Continue, and Distribute — Server as Record, GM as Arbiter               |
| [20260829-053](./20260829-053-generated-engine-sdk-and-status-presentation-split.md)  | Accepted | Generated Engine SDK, and the ECS/React Split for Status Presentation                         |
| [20260830-054](./20260830-054-interaction_effect_contribution_seam.md)               | Accepted | The Interaction Effect Contribution Seam                                                      |
| [20260901-055](./20260901-055-engine_wasm_build_toolchain.md)                         | Accepted | `wasm-pack` Remains the Engine's Build Driver, and Cargo Features Reach It Through `--`       |
| [20260901-056](./20260901-056-party_tokens_across_a_scene_change.md)                  | Accepted | Player Tokens Are Re-Created on Arrival, Not Carried                                          |
| [20260901-057](./20260901-057-actor_imagery_as_rows_keyed_by_role.md)                 | Accepted | Actor Imagery Is Rows Keyed by Role, Not Columns                                              |
| [20260901-058](./20260901-058-item_price_is_presentational.md)                        | Accepted | A Game Master's Item Price Is Presentational; Systems Own Economies                           |
| [20260902-059](./20260902-059-interface_pack_is_data_not_a_module.md) | Accepted | An Interface Pack Is Data, Not a Module |
| [20260902-060](./20260902-060-one_system_contract_with_declared_values.md) | Accepted | One System Contract, Carrying Declared Values |
| [20260902-061](./20260902-061-system_rules_discovery_not_a_registry.md) | Accepted | A System Pack's Rules Are Discovered, Not Listed |
| [20260902-062](./20260902-062-packs_extend_the_engine_with_data_not_code.md) | Accepted | System Packs Extend the Engine With Data, Not Code |
| [20260903-063](./20260903-063-a_pack_owns_the_tables_it_writes.md) | Accepted | A Pack Owns the Tables It Writes |
| [20260903-064](./20260903-064-ability_vocabulary_is_contributed.md) | Accepted | Ability Vocabulary Is Contributed, and the CHECK Constraint Goes |
| [20260903-065](./20260903-065-counted_acknowledgement_for_a_reversible_change.md) | Accepted | A Counted Acknowledgement Guards a Change That Looks Destructive |
| [20260904-066](./20260904-066-a_bundled_pack_ships_its_own_web_surfaces.md) | Accepted | A Bundled Pack Ships Its Own Web Surfaces, Found at Build Time |

> **Note (2026-08-19):** ADRs 020–024 originally collided with an unrelated "pack system" batch that reused the same day-020 through day-024 numbers. The world-domain ADRs (020–024 above) were committed first and are documented here; the colliding pack-system ADRs were renumbered to 026–030. See `docs/SYSTEM_HOOKS_API_GUIDE.md` and ADR-036's "Related Decisions" for the corrected references.

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
