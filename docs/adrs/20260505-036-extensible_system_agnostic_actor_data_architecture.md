# ADR-036: Extensible System-Agnostic Actor Data Architecture

**Status:** Accepted

**Decision Date:** 2026-05-05

---

## Context

ThunderForgeVTT must support unlimited game systems (D&D 5e, Pathfinder 2e, Call of Cthulhu 7e, Savage Worlds, custom homebrews) without requiring database schema migrations or code recompilation. Each system has fundamentally different actor attributes, mechanics, and rules:

- **D&D 5e**: Ability scores (STR, DEX, CON, INT, WIS, CHA), proficiencies, spell slots, class/level
- **Pathfinder 2e**: Ability modifiers, skills with proficiency tiers, saves, resonance points
- **CoC 7e**: Characteristics (STR, CON, SIZ, INT, POW, DEX, APP), skills with percentages, sanity
- **Savage Worlds**: Attributes, skills, edges, hindrances, power points
- **Custom Homebrew**: User-defined attributes and rules

**Previous Naive Approaches (Anti-Patterns):**

1. **Per-System Tables** ❌
   ```sql
   -- Creates rigid schema that breaks when adding systems
   world_actor_dnd5e_data (actor_id, strength, dexterity, ...)
   world_actor_pathfinder2e_data (actor_id, str_mod, skill_data, ...)  -- Different columns!
   world_actor_coc7_data (actor_id, san, pow, str, ...)  -- Yet another schema!
   ```
   - Problems: N systems = N tables + N ORM models + N GraphQL types
   - Adding system = database migration (breaks production)
   - Storing user homebrew requires code deployment

2. **Generic JSON Blob** ❌
   ```sql
   world_actor_data (actor_id, game_system_id, raw_json)
   ```
   - Problems: No schema validation, impossible to query ("find all elves"), no indexes, untyped

3. **Per-System GraphQL Mutations** ❌
   ```graphql
   mutation updateDnd5eAbilityData(...)
   mutation updatePathfinder2eAbilityData(...)
   mutation updateCoc7AbilityData(...)
   ```
   - Problems: Code duplication, N systems = N mutations, schema change per system

## Decision

We have decided to implement a **type-indexed JSONB architecture** with **semantic partitioning** and **manifest-driven validation**, enabling:

- ✅ Zero database migrations when adding new systems
- ✅ One GraphQL mutation supporting all systems
- ✅ System rules loaded from manifest (JSON schema), not code
- ✅ Queryable fields while maintaining flexibility
- ✅ Automatic schema evolution via manifest versioning

### Architecture Overview

#### Layer 1: Universal Actor Identity (world_actors)

```sql
CREATE TABLE world_actors (
  id UUID PRIMARY KEY,
  world_id UUID NOT NULL,
  scene_id UUID NOT NULL,
  game_system_id VARCHAR NOT NULL,  -- 'dnd5e', 'pathfinder2e', 'coc7', etc.
  
  -- Universal fields (all systems)
  name VARCHAR NOT NULL,
  created_by UUID NOT NULL,
  updated_by UUID NOT NULL,
  created_at TIMESTAMP NOT NULL,
  updated_at TIMESTAMP NOT NULL,
  
  FOREIGN KEY (created_by) REFERENCES users(id),
  FOREIGN KEY (updated_by) REFERENCES users(id)
);
```

**Purpose**: Actor identity, ownership metadata, system linkage. Works for any actor type (player character, NPC, hazard, prop, vehicle).

#### Layer 2: System-Specific Data (world_actor_system_data)

```sql
CREATE TABLE world_actor_system_data (
  id UUID PRIMARY KEY,
  actor_id UUID NOT NULL UNIQUE,
  game_system_id VARCHAR NOT NULL,
  
  -- Type-indexed JSONB columns (partition by data category, not system)
  ability_data JSONB,          -- { "strength": 10, "dexterity": 12, ... }
  resource_data JSONB,         -- { "hp": 25, "mana": 0, "san": 50, ... }
  proficiency_data JSONB,      -- { "skills": { "Acrobatics": true }, ... }
  trait_data JSONB,            -- { "class": "Wizard", "feats": [...], ... }
  spell_data JSONB,            -- { "known": [...], "slots": {...}, ... }
  
  created_by UUID NOT NULL,
  updated_by UUID NOT NULL,
  created_at TIMESTAMP NOT NULL,
  updated_at TIMESTAMP NOT NULL,
  
  FOREIGN KEY (actor_id) REFERENCES world_actors(id) ON DELETE CASCADE,
  FOREIGN KEY (created_by) REFERENCES users(id),
  FOREIGN KEY (updated_by) REFERENCES users(id)
);

-- Indexes for queryability
CREATE INDEX idx_world_actor_system_data_actor_id ON world_actor_system_data(actor_id);
CREATE INDEX idx_world_actor_system_data_game_system_id ON world_actor_system_data(game_system_id);
CREATE INDEX idx_world_actor_system_data_ability_data ON world_actor_system_data USING GIN(ability_data);
CREATE INDEX idx_world_actor_system_data_resource_data ON world_actor_system_data USING GIN(resource_data);
```

**Why This Works**:
- ✅ D&D 5e abilities: `{strength: 10, dexterity: 12, ...}`
- ✅ Pathfinder 2e: `{str_mod: 0, dex_mod: 2, ...}`
- ✅ CoC 7e: `{STR: 65, CON: 60, ...}`
- ✅ Custom systems: Any JSON structure fits

**Key Insight**: Systems differ in JSON *structure*, not in data *categories*. All systems have abilities (whatever they're called), resources, proficiencies, traits, and spells/abilities. **Partition by category, not by system.**

#### Layer 3: Rendering (world_tokens)

```sql
CREATE TABLE world_tokens (
  id UUID PRIMARY KEY,
  actor_id UUID,  -- NULL for static tokens (hazards, props)
  world_id UUID NOT NULL,
  scene_id UUID NOT NULL,
  
  -- Rendering data (system-agnostic)
  x FLOAT NOT NULL,
  y FLOAT NOT NULL,
  z INT NOT NULL,
  color VARCHAR NOT NULL,
  icon VARCHAR,
  rotation FLOAT,
  visible_to_players BOOLEAN,
  
  created_by UUID NOT NULL,
  updated_by UUID NOT NULL,
  created_at TIMESTAMP NOT NULL,
  updated_at TIMESTAMP NOT NULL
);
```

**Purpose**: Grid position, appearance, visibility. One actor can have multiple tokens (e.g., shadow double, summoned clone).

### Generic Mutation Design

```graphql
mutation updateActorSystemData(
  $actor_id: String!
  $game_system_id: String!
  $data_type: String!           # 'ability_data' | 'resource_data' | 'proficiency_data' | 'trait_data' | 'spell_data'
  $data: JSON!                   # { strength: 15 } or { skill_data: {...} }
) {
  updateActorSystemData(input: {
    actor_id: $actor_id
    game_system_id: $game_system_id
    data_type: $data_type
    data: $data
  }) {
    actor {
      id
      name
      game_system_id
    }
    data  # Updated JSONB
  }
}
```

**Resolver Flow**:
1. Extract auth user from context (never trust client-provided user ID)
2. Verify actor ownership: `WHERE created_by = auth_user.id`
3. Load system manifest via `GameSystemRegistry`
4. Validate JSON against manifest schema for `data_type`
5. Execute UPSERT on appropriate JSONB column
6. Create `world_events` entry for audit trail
7. Trigger `pg_notify` for real-time broadcasts
8. Return updated actor and data

### System Manifest Schema

Each system provides a `system.json` manifest:

```json
{
  "id": "dnd5e",
  "name": "D&D 5e",
  "version": "2.0",
  
  "data_types": {
    "ability_data": {
      "type": "object",
      "properties": {
        "strength": { "type": "integer", "minimum": 3, "maximum": 20 },
        "dexterity": { "type": "integer", "minimum": 3, "maximum": 20 },
        "constitution": { "type": "integer", "minimum": 3, "maximum": 20 },
        "intelligence": { "type": "integer", "minimum": 3, "maximum": 20 },
        "wisdom": { "type": "integer", "minimum": 3, "maximum": 20 },
        "charisma": { "type": "integer", "minimum": 3, "maximum": 20 }
      },
      "required": ["strength", "dexterity", "constitution", "intelligence", "wisdom", "charisma"]
    },
    
    "resource_data": {
      "type": "object",
      "properties": {
        "hp": { "type": "integer", "minimum": 0 },
        "ac": { "type": "integer", "minimum": 0 },
        "speed": { "type": "integer", "minimum": 0 },
        "inspiration": { "type": "integer", "minimum": 0 }
      }
    },
    
    "proficiency_data": {
      "type": "object",
      "additionalProperties": true  # Flexible proficiency tracking
    },
    
    "trait_data": {
      "type": "object",
      "properties": {
        "class": { "type": "string" },
        "level": { "type": "integer", "minimum": 1, "maximum": 20 },
        "background": { "type": "string" }
      },
      "required": ["class", "level"]
    },
    
    "spell_data": {
      "type": "object",
      "properties": {
        "known": { "type": "array", "items": { "type": "string" } },
        "prepared": { "type": "array", "items": { "type": "string" } },
        "slots": { "type": "object" }
      }
    }
  },

  "components": {
    "AbilityScores": "@/systems/dnd5e/components/AbilityScores.tsx",
    "SkillsList": "@/systems/dnd5e/components/SkillsList.tsx",
    "CharacterSheet": "@/systems/dnd5e/components/CharacterSheet.tsx"
  },

  "calculators": {
    "abilityModifier": "Math.floor((score - 10) / 2)",
    "proficiencyBonus": "Math.ceil(level / 4) + 1"
  }
}
```

**Purpose**: Defines validation schemas, component locations, and derived data calculators. Loaded at runtime—no code changes needed for new systems.

### Validator Registry (Rust Backend)

```rust
pub struct GameSystemRegistry {
    systems: Arc<DashMap<String, SystemManifest>>,
}

impl GameSystemRegistry {
    pub fn register(&self, system_id: &str, manifest: SystemManifest) {
        self.systems.insert(system_id.to_string(), manifest);
    }

    pub fn validate_actor_data(
        &self,
        game_system_id: &str,
        data_type: &str,
        data: &serde_json::Value,
    ) -> Result<(), ValidationError> {
        let manifest = self.systems
            .get(game_system_id)
            .ok_or(ValidationError::UnknownSystem)?;

        let schema = manifest.data_types
            .get(data_type)
            .ok_or(ValidationError::UnknownDataType)?;

        // Generic JSON schema validation
        validate_json_against_schema(data, schema)?;
        Ok(())
    }
}
```

**Benefit**: Validators are loaded from manifest schemas, not hardcoded in binary. Adding Pathfinder 2e = add `packs/systems/pathfinder2e/system.json`, zero code changes.

### RxDB Real-Time Sync

```typescript
// Frontend collection schema (generic for all systems)
const worldActorSystemDataCollection: RxJsonSchema = {
  type: 'object',
  primaryKey: 'id',
  properties: {
    id: { type: 'string' },
    actor_id: { type: 'string', indexed: true },
    game_system_id: { type: 'string', indexed: true },
    ability_data: { type: 'object', nullable: true },
    resource_data: { type: 'object', nullable: true },
    proficiency_data: { type: 'object', nullable: true },
    trait_data: { type: 'object', nullable: true },
    spell_data: { type: 'object', nullable: true },
  }
};
```

**Circular Flow**:
```
User Interaction (React UI)
    ↓
Optimistic RxDB Update (instant feedback)
    ↓
GraphQL Mutation (updateActorSystemData)
    ↓
Server Validation (manifest schema)
    ↓
Database Update + Audit Event
    ↓
pg_notify Broadcast
    ↓
GraphQL Subscription (worldActorSystemDataUpdated)
    ↓
RxDB Sync (canonical state)
    ↓
React Re-render (confirmed data)
    ↓
(OR: Rollback if server rejects)
```

### Optimistic Updates with Rollback

```typescript
const { mutate } = useUpdateActorData();

const handleAbilityChange = async (ability: string, value: number) => {
  // 1. Save original state
  const original = actorData.ability_data;

  // 2. Optimistic update (instant UI feedback)
  await db.collections.worldActorSystemData.upsert({
    ...actorData,
    ability_data: { ...original, [ability]: value }
  });

  // 3. Send mutation to server
  try {
    await mutate({
      actor_id: actorId,
      game_system_id: 'dnd5e',
      data_type: 'ability_data',
      data: { [ability]: value }
    });
  } catch (error) {
    // 4. Rollback on rejection
    await db.collections.worldActorSystemData.upsert({
      ...actorData,
      ability_data: original
    });
    showError(`Invalid update: ${error.message}`);
  }
};
```

**Key Pattern**: Users see instant feedback (RxDB), mutations validate async (GraphQL), rollback on rejection.

### React System-Aware Components

```typescript
// Generic component that works for any system
export const CharacterSheet: React.FC<{ actor_id: string }> = ({ actor_id }) => {
  const { data, loading } = useActorSystemData(actor_id);
  const { manifest } = useGameSystemManifest(data?.game_system_id);

  if (!manifest) return <Loading />;

  return (
    <div>
      {/* Render only fields defined in manifest */}
      {manifest.data_types.ability_data && (
        <AbilityScores ability_data={data.ability_data} />
      )}
      
      {manifest.data_types.resource_data && (
        <ResourceBar resource_data={data.resource_data} />
      )}
      
      {manifest.components.CharacterSheet && (
        <DynamicComponent
          component={manifest.components.CharacterSheet}
          data={data}
        />
      )}
    </div>
  );
};
```

**Benefit**: Same React component renders correctly for D&D 5e, Pathfinder 2e, CoC 7e, homebrews—manifest drives everything.

---

## Rationale

### Y-Statements

**Y1: Type-Indexed JSONB (Not Per-System Tables)**

We need a way to store fundamentally different actor attributes across unlimited game systems **without requiring database migrations or schema changes**. We chose **type-indexed JSONB columns** (ability_data, resource_data, proficiency_data, trait_data, spell_data) **because** different systems use different JSON structures but share the same data *categories* **and this allows new systems to be added to production without any database changes**, whereas per-system tables would require migrations for each new system, breaking production deployments.

**Y2: Manifest-Driven Validation (Not Hardcoded Validators)**

We need a way to validate actor data according to system-specific rules **without recompiling the binary or deploying new code for each system**. We chose **loading validation schemas from JSON manifests and performing generic JSON schema validation** **because** system rules naturally live as data (in manifest.json), not in code **and this allows users to add custom homebrew systems by uploading JSON at runtime**, whereas hardcoded validators require code changes and recompilation.

**Y3: Generic GraphQL Mutations (Not Per-System Mutations)**

We need a single GraphQL API that works for all present and future game systems **without adding new mutation types for each system**. We chose **a single `updateActorSystemData` mutation with `data_type` parameter** **because** routing by data type (ability_data, resource_data, etc.) works for all systems regardless of their specific attribute names **and this means the GraphQL API never changes when adding systems**, whereas per-system mutations (updateDnd5eAbilityData, updatePathfinder2eAbilityData, etc.) requires schema changes per system.

**Y4: Optimistic Updates with Rollback (Not Pessimistic Waits)**

We need users to see instant UI feedback when updating actor data (network latency is 50-200ms) **while ensuring consistency with server-approved canonical state**. We chose **optimistic RxDB updates followed by async GraphQL mutations, with automatic rollback on rejection** **because** this provides immediate visual feedback while maintaining consistency **and users see failures only when server rejects, not on every network delay**, whereas pessimistic approaches require users to wait for network round-trips before seeing any feedback.

**Y5: Lazy-Loaded System Manifests (Not Global Registry)**

We need to support 20+ game systems simultaneously **without bloating memory or requiring all systems to be loaded at startup**. We chose **lazy-loading system manifests from JSON files in React Context, with in-memory caching** **because** only active systems need to be loaded **and this keeps memory footprint minimal while supporting unlimited systems**, whereas pre-loading all manifests at startup would be slow and memory-intensive.

### Trade-Offs

| Trade-Off | Choice | Why |
|-----------|--------|-----|
| **Schema Flexibility vs. Queryability** | Use GIN indexes on JSONB columns | Allows arbitrary JSON while maintaining indexed queries ("find all strength > 15") |
| **Type Safety vs. Runtime Flexibility** | TypeScript interfaces for manifest, generic JSON validation | Catches manifest errors at development time, supports user-defined homebrew at runtime |
| **Server-Side Validation vs. Client-Side** | Server always validates against manifest schema | Prevents malicious clients from bypassing rules; manifest is source of truth |
| **Eager Loading vs. Lazy Loading** | Lazy-load manifests on demand | Supports 20+ systems without memory bloat; only active systems consume RAM |
| **Audit Trail Granularity** | Record each mutation as world_event | Enables full undo/redo, conflict resolution, and rollback; storage cost justified by consistency |

---

## Consequences

### Positive

✅ **Zero Database Migrations for New Systems**
- Add Pathfinder 2e: Create `packs/systems/pathfinder2e/system.json`, zero SQL changes
- Add CoC 7e: Create `packs/systems/coc7/system.json`, zero SQL changes
- Add custom homebrew: User uploads JSON manifest, zero code changes

✅ **Single GraphQL Mutation Supports All Systems**
- One mutation works forever, regardless of system count
- No schema bloat (no `updateDnd5eAbilityData`, `updatePathfinder2eAbilityData`, etc.)
- System rules loaded from manifest, not code

✅ **Instant User Feedback**
- Optimistic updates reflected in UI immediately (50ms vs. 200ms network latency)
- Rollback on rejection prevents stale state
- Users only see failures when server actually rejects

✅ **Queryable System Data**
- GIN indexes on JSONB columns enable `ability_data->>'strength' > 15` queries
- Search across all systems: "find all actors with charisma < 8"
- Analytics queries possible (e.g., "average ability distribution by system")

✅ **Extensible Without Code Changes**
- New validators added to manifest schema, not Rust code
- New React components added to manifest, not package.json
- Support unlimited game systems in single codebase

### Negative

❌ **JSON Schema Complexity**
- Manifest schemas must be authored carefully (easy to introduce bugs)
- Validation errors from schema mismatches can be confusing
- Mitigation: Provide schema templates and validation tooling

❌ **JSONB Storage Overhead**
- JSONB has 20-30% storage overhead vs. relational tables
- For massive actor counts (100K+), could impact disk usage
- Mitigation: Compress inactive actor data, archive old records

❌ **Limited Relational Queries**
- Can't do complex joins across systems (e.g., find elves with intelligence > 15 in D&D specifically)
- JSONB queries less efficient than relational indexes
- Mitigation: Denormalize frequently-queried fields, use materialized views

❌ **Manifest Versioning Complexity**
- Schema changes to ability_data for D&D 5e 2.1 require migration strategy
- Old characters may not validate against new schema
- Mitigation: Include schema versioning in manifest, support multiple versions

❌ **Derived Data Synchronization**
- Ability modifiers, proficiency bonuses must be calculated locally
- If calculation logic changes between server/client, inconsistencies appear
- Mitigation: Store derived data in database on mutation, not calculated client-side

---

## Implementation

### Phase A: Database Schema ✅
- [x] Create `world_actors` table (universal, system-agnostic)
- [x] Create `world_actor_system_data` table (system-specific JSONB)
- [x] Create `world_tokens` table (rendering layer)
- [x] Add 14 indexes for performance

### Phase B: D&D 5e Validators ✅
- [x] Create manifest schema for 5 semantic data types
- [x] Implement 5 validators (ability, resource, proficiency, trait, spell)
- [x] Add 45+ unit test cases

### Phase C: GraphQL Mutations ✅
- [x] Implement generic `updateActorSystemData` mutation
- [x] Add ownership validation (ADR-010)
- [x] Create world_events audit trail
- [x] Setup pg_notify trigger

### Phase D: RxDB Integration ✅
- [x] Create world_actor_system_data collection
- [x] Implement bidirectional replication
- [x] Add optimistic update + rollback logic

### Phase E: React Components ✅
- [x] Implement `useActorSystemData` hook (RxDB subscriptions)
- [x] Implement `useUpdateActorData` hook (mutations + rollback)
- [x] Implement `GameSystemContext` (lazy manifest loading)
- [x] Build AbilityScores, SkillsList, CharacterSheet components

### Phase F: Testing & Validation ✅
- [x] 45+ validator unit tests
- [x] 12+ registry validation tests
- [x] 10+ E2E scenario tests
- [x] Full monorepo build validation (0 errors)

---

## Related Decisions

- **ADR-000**: Durable Objects via GraphQL Event-Driven Synchronization (parent architecture)
- **ADR-010**: Ownership Fields on Persisted Tables (created_by/updated_by enforcement)
- **ADR-009**: Created-By / Updated-By Enforcement (ownership validation)
- **ADR-027**: Game System Packaging and Manifest Contract (system manifest standard)

---

## References

### Design Patterns
- [Event Sourcing Pattern](https://martinfowler.com/eaaDev/EventSourcing.html) - Audit trail via world_events
- [PostgreSQL JSONB Guide](https://www.postgresql.org/docs/current/datatype-json.html) - Type-indexed storage
- [JSON Schema Validation](https://json-schema.org/) - Manifest schema standard
- [Figma's Multiplayer Architecture](https://www.figma.com/blog/how-figmas-multiplayer-technology-works/) - Event-driven sync pattern
- [PlayCanvas Multiplayer Systems](https://playcanvas.com/) - System-agnostic game data

### Implementation References
- [RxDB Replication Plugin](https://rxdb.info/replication.html) - Frontend sync
- [Diesel ORM JSONB Support](https://docs.diesel.rs/master/diesel/expression/expression_methods/struct.ExpressionMethods.html#method.assume_not_null) - Backend persistence
- [Axum GraphQL Integration](https://github.com/tokio-rs/axum) - Server implementation
- [React Context for Manifest Caching](https://react.dev/reference/react/useContext) - Component system

---

## Summary

By adopting **type-indexed JSONB with manifest-driven validation**, ThunderForgeVTT can scale to unlimited game systems without database migrations, code recompilation, or GraphQL schema changes. The architecture supports both built-in systems (D&D 5e) and runtime user-defined systems (homebrew), with optimistic UI updates and guaranteed consistency via pg_notify pub/sub. This decision positions the platform for indefinite expansion while maintaining simplicity and performance.
