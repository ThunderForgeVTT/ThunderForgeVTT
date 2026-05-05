# System Hooks API Guide

## Overview

ThunderForgeVTT uses a **system hooks API** to allow game systems (like D&D 5e, Pathfinder, etc.) to customize core VTT behavior without modifying the engine code.

Systems are packaged as ZIP files containing:
- **manifest.json** - System metadata and hook declarations
- **module/** - JavaScript ESM modules implementing hooks
- **styles/** - CSS files for system-specific theming
- **packs/** - Data packs (actors, items, spells, etc.)

---

## System Structure

```
d20-5e.zip
├── system.json                      # Manifest file
├── module/
│   ├── main.mjs                     # Main module with hook implementations
│   ├── rolls.mjs                    # Dice rolling module
│   └── conditions.mjs               # Condition handling
├── styles/
│   ├── d20-theme.css               # System theme
│   └── character-sheet.css
└── packs/
    ├── actors.json                  # Actor templates
    └── items.json                   # Item templates
```

---

## 1. System Manifest (system.json)

The manifest declares system metadata and which hooks are implemented.

```json
{
  "id": "d20-5e",
  "title": "D&D 5th Edition",
  "description": "Official D&D 5th Edition rules for ThunderForge VTT",
  "authors": [
    {
      "name": "Author Name",
      "email": "author@example.com"
    }
  ],
  "version": "0.1.0",
  "compatibility": {
    "minimum": "0.1.0",
    "verified": "0.1.0"
  },
  "esmodules": [
    "module/main.mjs",
    "module/rolls.mjs"
  ],
  "styles": [
    "styles/d20-theme.css",
    "styles/character-sheet.css"
  ],
  "packs": [],
  "media": {
    "logo": "media/d20-logo.svg"
  }
}
```

**Key Fields:**
- `id` (string, required) - Unique system ID, lowercase alphanumeric + `-_.`
- `title` (string, required) - Display name
- `version` (string, required) - Semantic version (major.minor.patch)
- `compatibility` (object, required) - ThunderForge version requirements
  - `minimum` - Minimum required core version
  - `verified` - Recommended/tested core version
- `esmodules` (array) - JavaScript files to load
- `styles` (array) - CSS files to load
- `authors` (array) - System authors

---

## 2. Hook Contract

Systems implement hooks by exporting functions matching this interface:

```typescript
interface SystemHooksContract {
  computeDerivedStats?(baseStats: BaseTokenStats): Promise<DerivedTokenStats>;
  onTokenMove?(params: TokenMoveParams): Promise<boolean>;
  validateRoll?(params: DiceRollParams): Promise<DiceRollResult>;
  formatDamage?(params: DamageFormatParams): Promise<string>;
  onConditionChange?(params: ConditionChangeParams): Promise<boolean>;
  checkTokenVisibility?(params: TokenVisibilityParams): Promise<boolean>;
  computeArmorClass?(baseStats: BaseTokenStats): Promise<number>;
}
```

### 2.1 `computeDerivedStats`

**Purpose:** Compute derived statistics from base stats (AC, initiative, etc.)

**Signature:**
```typescript
(baseStats: BaseTokenStats) => Promise<DerivedTokenStats>
```

**Parameters:**
```typescript
interface BaseTokenStats {
  health: number;
  maxHealth: number;
  strength: number;
  dexterity: number;
  constitution: number;
  intelligence: number;
  wisdom: number;
  charisma: number;
  [key: string]: number | string;  // System-specific stats
}
```

**Returns:**
```typescript
interface DerivedTokenStats {
  armorClass?: number;
  initiative?: number;
  healthPercentage?: number;
  isDead?: boolean;
  isFullHealth?: boolean;
  [key: string]: any;
}
```

**Example (D&D 5e):**
```javascript
function computeDerivedStats(baseStats) {
  const dexMod = Math.floor((baseStats.dexterity - 10) / 2);
  const conMod = Math.floor((baseStats.constitution - 10) / 2);

  return {
    armorClass: 10 + dexMod,
    initiative: dexMod,
    healthPercentage: (baseStats.health / baseStats.maxHealth) * 100,
    isDead: baseStats.health <= 0,
    proficiencyBonus: 2,
  };
}
```

---

### 2.2 `onTokenMove`

**Purpose:** Validate or reject token movement

**Signature:**
```typescript
(params: TokenMoveParams) => Promise<boolean>
```

**Parameters:**
```typescript
interface TokenMoveParams {
  tokenId: string;
  x: number;          // New position
  y: number;
  sceneId: string;
  currentX: number;   // Current position
  currentY: number;
}
```

**Returns:** `true` to allow move, `false` to reject

**Example:**
```javascript
function onTokenMove(params) {
  // Reject if moving more than 30 feet per round
  const distance = Math.sqrt(
    Math.pow(params.x - params.currentX, 2) +
    Math.pow(params.y - params.currentY, 2)
  );
  
  if (distance > 30 * 5) {  // 5 feet per grid square
    console.warn('Movement exceeds speed limit');
    return false;
  }
  
  return true;
}
```

---

### 2.3 `validateRoll`

**Purpose:** Validate and parse dice roll notation

**Signature:**
```typescript
(params: DiceRollParams) => Promise<DiceRollResult>
```

**Parameters:**
```typescript
interface DiceRollParams {
  diceStr: string;      // "4d6", "2d20kh1", "1d20+5"
  modifier?: number;    // Additional modifier
}
```

**Returns:**
```typescript
interface DiceRollResult {
  valid: boolean;
  error?: string;
  dice: string;
  modifier?: number;
}
```

**Example:**
```javascript
function validateRoll(params) {
  const regex = /^(\d+)d(\d+)([kh])?([lh])?(\d+)?(\+|-)?(\d+)?$/i;
  
  if (!params.diceStr.match(regex)) {
    return {
      valid: false,
      error: `Invalid dice notation: ${params.diceStr}`,
      dice: params.diceStr,
    };
  }

  return {
    valid: true,
    dice: params.diceStr,
    modifier: params.modifier || 0,
  };
}
```

---

### 2.4 `formatDamage`

**Purpose:** Format damage output for display (e.g., "2d6+3 (avg: 10)")

**Signature:**
```typescript
(params: DamageFormatParams) => Promise<string>
```

**Parameters:**
```typescript
interface DamageFormatParams {
  diceStr: string;  // "2d6+3"
}
```

**Returns:** Formatted string

**Example:**
```javascript
function formatDamage(params) {
  const match = params.diceStr.match(/^(\d+)d(\d+)(?:\+(\d+))?/);
  if (!match) return params.diceStr;

  const numDice = parseInt(match[1]);
  const diceSize = parseInt(match[2]);
  const modifier = parseInt(match[3] || 0);

  const average = Math.round((diceSize + 1) / 2 * numDice) + modifier;
  return `${params.diceStr} (avg: ${average})`;
}
```

---

### 2.5 `onConditionChange`

**Purpose:** Validate or prevent condition application

**Signature:**
```typescript
(params: ConditionChangeParams) => Promise<boolean>
```

**Parameters:**
```typescript
interface ConditionChangeParams {
  tokenId: string;
  condition: string;    // "poisoned", "paralyzed", etc.
  applied: boolean;     // true to apply, false to remove
}
```

**Returns:** `true` to allow, `false` to prevent

**Example:**
```javascript
function onConditionChange(params) {
  // Prevent applying "dead" condition (only health can determine this)
  if (params.condition === 'dead') {
    return false;
  }
  
  return true;
}
```

---

### 2.6 `checkTokenVisibility`

**Purpose:** Determine if one token can see another (fog of war)

**Signature:**
```typescript
(params: TokenVisibilityParams) => Promise<boolean>
```

**Parameters:**
```typescript
interface TokenVisibilityParams {
  fromTokenId: string;
  toTokenId: string;
  fogMask?: string;     // Base64 or URL to fog bitmap
}
```

**Returns:** `true` if visible, `false` if hidden

**Example:**
```javascript
function checkTokenVisibility(params) {
  // Always visible to self
  if (params.fromTokenId === params.toTokenId) {
    return true;
  }
  
  // Check fog bitmap (simplified)
  if (params.fogMask) {
    // Implement vision/fog logic here
    return true;
  }
  
  return false;
}
```

---

### 2.7 `computeArmorClass`

**Purpose:** Compute armor class from base stats

**Signature:**
```typescript
(baseStats: BaseTokenStats) => Promise<number>
```

**Example:**
```javascript
function computeArmorClass(baseStats) {
  // 10 + DEX modifier
  const dexMod = Math.floor((baseStats.dexterity - 10) / 2);
  return 10 + dexMod;
}
```

---

## 3. Hook Implementation Example

Full D&D 5e system example (module/main.mjs):

```javascript
/**
 * D&D 5th Edition System Module
 */

function computeDerivedStats(baseStats) {
  const abilities = ['strength', 'dexterity', 'constitution', 'intelligence', 'wisdom', 'charisma'];
  const mods = {};

  abilities.forEach(ability => {
    mods[`${ability}Mod`] = Math.floor((baseStats[ability] - 10) / 2);
  });

  return {
    ...mods,
    armorClass: 10 + mods.dexterityMod,
    initiative: mods.dexterityMod,
    healthPercentage: (baseStats.health / baseStats.maxHealth) * 100,
    isDead: baseStats.health <= 0,
    proficiencyBonus: 2,
  };
}

function onTokenMove(params) {
  // Allow all moves (validation at world level)
  return true;
}

function validateRoll(params) {
  // D&D 5e supports: 4d6, 2d20kh1, 1d20+5, etc.
  const regex = /^(\d+)d(\d+)([kh])?([lh])?(\d+)?(\+|-)?(\d+)?$/i;
  
  return {
    valid: !!params.diceStr.match(regex),
    error: !params.diceStr.match(regex) ? `Invalid: ${params.diceStr}` : undefined,
    dice: params.diceStr,
  };
}

function formatDamage(params) {
  const match = params.diceStr.match(/^(\d+)d(\d+)(?:\+(\d+))?/);
  if (!match) return params.diceStr;

  const numDice = parseInt(match[1]);
  const diceSize = parseInt(match[2]);
  const modifier = parseInt(match[3] || 0);

  const average = Math.round(numDice * (diceSize + 1) / 2) + modifier;
  return `${params.diceStr} (avg: ${average})`;
}

function onConditionChange(params) {
  // Prevent "dead" - only health determines this
  return params.condition !== 'dead';
}

function checkTokenVisibility(params) {
  // Simplified: always visible (full fog logic deferred)
  return true;
}

function computeArmorClass(baseStats) {
  const dexMod = Math.floor((baseStats.dexterity - 10) / 2);
  return 10 + dexMod;
}

// Export all hooks
export default {
  computeDerivedStats,
  onTokenMove,
  validateRoll,
  formatDamage,
  onConditionChange,
  checkTokenVisibility,
  computeArmorClass,
};
```

---

## 4. Using Hooks in React Components

Components can access system hooks via the `useSystemHooks` hook:

```typescript
import { useSystemHooks, useSystemHook, computeTokenDerivedStats } from '@/hooks/useSystemHooks';

export function TokenStats({ token }) {
  const { hooks, loading } = useSystemHooks();

  if (loading) return <div>Loading system...</div>;

  // Compute derived stats using system hooks
  const derived = computeTokenDerivedStats(token.baseStats, hooks);

  return (
    <div>
      <p>AC: {derived.armorClass}</p>
      <p>Initiative: {derived.initiative}</p>
      <p>Health: {Math.round(derived.healthPercentage)}%</p>
    </div>
  );
}
```

---

## 5. System Installation & Loading

### Admin Installation

Admin users can install system packages via the web UI or API:

```bash
POST /api/admin/systems/install
Content-Type: multipart/form-data

package: <d20-5e.zip>
```

### Automatic Loading

When a world is loaded, the system is automatically fetched from:

```
GET /api/systems/{systemId}/manifest.json
GET /api/systems/{systemId}/module/main.mjs
GET /api/systems/{systemId}/styles/d20-theme.css
```

Modules are dynamically imported and hooks are registered in the React context.

---

## 6. Security Considerations

### Sandboxing

System modules run in the same context as the VTT. **Systems must be trusted**:

- ✅ Only admins can install systems
- ✅ Systems run in web worker context (deferred: Phase 4+)
- ✅ System code is never persisted to user data
- ⚠️ System authors have access to: tokens, scenes, hooks, network

### Best Practices

1. **Validate all inputs** in hooks
2. **Use try-catch** for async operations
3. **Don't modify external state** from hooks
4. **Return promises** for all async work
5. **Log errors** to console for debugging

---

## 7. Testing Systems Locally

### Setup

```bash
# Create system directory
mkdir -p ~/.thunderforge/systems/my-system/module
mkdir -p ~/.thunderforge/systems/my-system/styles

# Create manifest
cat > ~/.thunderforge/systems/my-system/system.json << EOF
{
  "id": "my-system",
  "title": "My System",
  "version": "0.1.0",
  ...
}
EOF

# Create module
cat > ~/.thunderforge/systems/my-system/module/main.mjs << EOF
export default {
  computeDerivedStats: (stats) => ({ ...stats }),
};
EOF
```

### Testing in Browser

1. Open DevTools (F12)
2. Check network tab: `/api/systems/{id}/*` requests
3. Check console for system loading errors
4. Verify hooks are available: `window.__systemHooks` (if exposed)

---

## 8. API Reference

### useSystemHooks

```typescript
function useSystemHooks(): {
  systemId?: string;
  hooks: SystemHooksContract;
  loading: boolean;
  error?: string;
  reload: () => Promise<void>;
}
```

### useSystemHook (single hook)

```typescript
function useSystemHook<T extends keyof SystemHooksContract>(
  hookName: T,
  params?: Parameters<SystemHooksContract[T]>[0],
): {
  loading: boolean;
  error?: string;
  result?: any;
}
```

### Helper Functions

```typescript
// Compute derived stats with fallback
computeTokenDerivedStats(baseStats, hooks?)

// Validate token movement
validateTokenMove(params, hooks?)

// Format dice roll
formatDiceRoll(diceStr, hooks?)
```

---

## 9. Roadmap

### Phase 3 (Current)
- [x] Static file server for system packages
- [x] Hook contract definition
- [x] React provider + hooks
- [x] Example D&D 5e system

### Phase 4
- [ ] Bevy system rules integration
- [ ] Token stat computation using hooks
- [ ] Condition system integration
- [ ] Dice roll engine

### Phase 5
- [ ] Web worker sandboxing
- [ ] System marketplace
- [ ] Dependency resolution
- [ ] Version compatibility checks

---

## 10. FAQ

**Q: Can systems modify game state directly?**  
A: No, hooks are read-only. Systems return computed values; the VTT applies changes.

**Q: Can systems use external APIs?**  
A: Yes, but CORS must be configured. Avoid calling non-HTTPS endpoints.

**Q: Can multiple systems be active?**  
A: Each world has one system. Switching requires creating a new world.

**Q: How do I distribute my system?**  
A: Package as ZIP and upload to the marketplace (Phase 5).

---

## Example Systems

See `/tmp/d20-5e/` for a complete minimal D&D 5e system.

---

## References

- [ADR-023: Runtime Module Loading & Security](../adrs/20260504-023-runtime_module_loading_and_security.md)
- [System Hooks TypeScript Types](../hooks/useSystemHooks.ts)
- [SystemHooksProvider React Component](../providers/SystemHooksProvider.tsx)
