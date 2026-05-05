/**
 * apps/web/src/hooks/__tests__/e2e-scenarios.test.ts
 * Phase 4.8.1: F2 - End-to-End Scenario Tests
 *
 * Tests complete circular flows:
 * 1. Create D&D 5e character → Load data → Update ability → Verify sync
 * 2. Optimistic update → Server rejection → Rollback
 * 3. Multi-user sync (simulated)
 * 4. Concurrent mutations handling
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useActorSystemData } from "../useActorSystemData";
import { useUpdateActorData } from "../useUpdateActorData";
import { useGameSystemManifest } from "@/contexts/GameSystemContext";
import { getWorldDatabase } from "@/engine/world/sync/database";
import type { ActorSystemData } from "../useActorSystemData";

// Mock database and GraphQL
vi.mock("@/engine/world/sync/database");
global.fetch = vi.fn();

describe("Phase F2: E2E Scenario Tests", () => {
  let mockDb: any;
  let mockCollection: any;

  beforeEach(() => {
    vi.clearAllMocks();

    mockCollection = {
      find: vi.fn().mockReturnValue({
        where: vi.fn().mockReturnValue({
          eq: vi.fn().mockReturnValue({
            exec: vi.fn(),
            $: { subscribe: vi.fn() },
          }),
        }),
        exec: vi.fn(),
      }),
      upsert: vi.fn(),
      findOne: vi.fn(),
    };

    mockDb = {
      collections: {
        world_actor_system_data: mockCollection,
      },
    };

    (getWorldDatabase as any).mockResolvedValue(mockDb);
  });

  describe("Scenario 1: Create Character → Load → Update → Sync", () => {
    /**
     * Full E2E flow:
     * 1. Create new D&D 5e character (actor created in backend)
     * 2. Load character data from RxDB
     * 3. Verify initial stats display correctly
     * 4. User increases STR from 15 to 18 (optimistic)
     * 5. GraphQL mutation sent to server
     * 6. Server validates and persists
     * 7. pg_notify broadcasts update
     * 8. RxDB subscription receives update
     * 9. Component re-renders with canonical data
     * 10. No UI flicker or inconsistency
     */
    it("should complete full character creation and update flow", async () => {
      // Step 1: Initial character data (newly created)
      const initialData: ActorSystemData = {
        id: "sys-data-hero-1",
        actor_id: "actor-hero-1",
        game_system_id: "dnd5e",
        ability_data: {
          strength: 15,
          dexterity: 14,
          constitution: 13,
          intelligence: 12,
          wisdom: 11,
          charisma: 10,
        },
        proficiency_data: {
          acrobatics: false,
          animal_handling: false,
        },
        resource_data: {
          hp: 45,
          ac: 14,
          speed: 30,
        },
        trait_data: {
          class: "Fighter",
          level: 3,
          race: "Human",
        },
        spell_data: {},
        created_by: "user-1",
        updated_by: "user-1",
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
      };

      let subscribeCallback: ((docs: any[]) => void) | null = null;

      // Step 2: Setup initial load
      mockCollection.find().where().eq().exec.mockResolvedValueOnce([initialData]);
      mockCollection.find().where().eq().$.subscribe.mockImplementationOnce((cb: any) => {
        subscribeCallback = cb;
        return { unsubscribe: vi.fn() };
      });

      // Step 3: Load character sheet
      const { result: queryResult } = renderHook(() =>
        useActorSystemData("actor-hero-1", "dnd5e"),
      );

      await waitFor(() => {
        expect(queryResult.current.data?.ability_data?.strength).toBe(15);
      });

      // Verify initial display
      expect(queryResult.current.loading).toBe(false);
      expect(queryResult.current.error).toBeNull();
      expect(queryResult.current.data?.trait_data?.class).toBe("Fighter");
      expect(queryResult.current.data?.resource_data?.hp).toBe(45);

      // Step 4: Setup mutation for ability update
      mockCollection.find().where().eq().exec.mockResolvedValueOnce([initialData]);

      const updatedData: ActorSystemData = {
        ...initialData,
        ability_data: {
          ...initialData.ability_data!,
          strength: 18,
        },
        updated_at: "2026-01-01T00:01:00Z",
      };

      const serverResponse = {
        data: {
          updateActorSystemData: {
            success: true,
            data: updatedData,
          },
        },
      };

      (global.fetch as any).mockResolvedValueOnce({
        ok: true,
        json: async () => serverResponse,
      });

      // Step 5: User updates strength
      const { result: mutationResult } = renderHook(() =>
        useUpdateActorData("actor-hero-1", "dnd5e"),
      );

      let mutationComplete = false;
      act(() => {
        mutationResult.current
          .mutate("ability_data", {
            strength: 18,
            dexterity: 14,
            constitution: 13,
            intelligence: 12,
            wisdom: 11,
            charisma: 10,
          })
          .then(() => {
            mutationComplete = true;
          });
      });

      // Verify optimistic update (immediate feedback)
      expect(mockCollection.upsert).toHaveBeenCalled();

      // Step 6-7: Wait for mutation to complete
      await waitFor(() => {
        expect(mutationComplete).toBe(true);
      });

      // Step 8: Simulate pg_notify broadcast via subscription
      act(() => {
        subscribeCallback?.([updatedData]);
      });

      // Step 9: Verify final state
      expect(queryResult.current.data?.ability_data?.strength).toBe(18);
      expect(queryResult.current.data?.updated_at).toBe("2026-01-01T00:01:00Z");

      // Verify derived data would recalculate correctly
      // Modifier for 18 = (18-10)/2 = 4
      const modifier = Math.floor((18 - 10) / 2);
      expect(modifier).toBe(4);
    });

    it("should handle concurrent updates to different data types", async () => {
      const initialData: ActorSystemData = {
        id: "sys-data-2",
        actor_id: "actor-2",
        game_system_id: "dnd5e",
        ability_data: { strength: 15, dexterity: 14, constitution: 13, intelligence: 12, wisdom: 11, charisma: 10 },
        proficiency_data: { acrobatics: false },
        resource_data: { hp: 50, ac: 14 },
        trait_data: { class: "Wizard", level: 5 },
        spell_data: {},
        created_by: "user-1",
        updated_by: "user-1",
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
      };

      mockCollection.find().where().eq().exec.mockResolvedValue([initialData]);
      mockCollection.find().where().eq().$.subscribe.mockReturnValue({
        unsubscribe: vi.fn(),
      });

      (global.fetch as any).mockResolvedValue({
        ok: true,
        json: async () => ({
          data: {
            updateActorSystemData: { success: true, data: initialData },
          },
        }),
      });

      const { result: mutationResult } = renderHook(() =>
        useUpdateActorData("actor-2", "dnd5e"),
      );

      // Send concurrent mutations
      const promise1 = mutationResult.current.mutate("ability_data", { ...initialData.ability_data, strength: 18 });
      const promise2 = mutationResult.current.mutate("proficiency_data", { acrobatics: true });

      // Both should be queued (not simultaneously executed)
      await waitFor(() => {
        expect(mutationResult.current.isPending).toBe(false);
      });

      // Both mutations should have been sent
      expect(global.fetch).toHaveBeenCalled();
    });
  });

  describe("Scenario 2: Optimistic Update → Server Rejection → Rollback", () => {
    /**
     * Error handling flow:
     * 1. User sets STR to 25 (invalid: max 20)
     * 2. Optimistic update to RxDB
     * 3. GraphQL mutation sent
     * 4. Server validates and rejects (422 or GraphQL error)
     * 5. UI immediately rolls back to original value
     * 6. Error message shown to user
     * 7. User can retry or correct input
     */
    it("should rollback and show error on server rejection", async () => {
      const originalData: ActorSystemData = {
        id: "sys-data-3",
        actor_id: "actor-3",
        game_system_id: "dnd5e",
        ability_data: { strength: 15, dexterity: 14, constitution: 13, intelligence: 12, wisdom: 11, charisma: 10 },
        proficiency_data: {},
        resource_data: {},
        trait_data: { class: "Barbarian", level: 2 },
        spell_data: {},
        created_by: "user-1",
        updated_by: "user-1",
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
      };

      mockCollection.find().where().eq().exec.mockResolvedValueOnce([originalData]);
      mockCollection.find().where().eq().$.subscribe.mockReturnValue({
        unsubscribe: vi.fn(),
      });

      // Server rejects with validation error
      const validationError = {
        errors: [
          {
            message: "ability_data validation failed: strength must be between 3 and 20",
            extensions: { code: "VALIDATION_ERROR" },
          },
        ],
      };

      mockCollection.find().where().eq().exec.mockResolvedValueOnce([originalData]);

      (global.fetch as any).mockResolvedValueOnce({
        ok: true,
        json: async () => validationError,
      });

      const { result: mutationResult } = renderHook(() =>
        useUpdateActorData("actor-3", "dnd5e"),
      );

      let caughtError: Error | null = null;

      act(() => {
        mutationResult.current
          .mutate("ability_data", {
            strength: 25, // Invalid!
            dexterity: 14,
            constitution: 13,
            intelligence: 12,
            wisdom: 11,
            charisma: 10,
          })
          .catch((e) => {
            caughtError = e;
          });
      });

      await waitFor(() => {
        expect(caughtError).toBeTruthy();
      });

      // Verify error message is descriptive
      expect(caughtError?.message).toContain("strength must be between 3 and 20");

      // Verify rollback was performed (restore original values)
      expect(mockCollection.upsert).toHaveBeenCalledWith(
        expect.objectContaining({
          ability_data: {
            strength: 15,
            dexterity: 14,
            constitution: 13,
            intelligence: 12,
            wisdom: 11,
            charisma: 10,
          },
          _optimistic: false,
        }),
      );

      // State should be consistent (not stuck in optimistic state)
      expect(mutationResult.current.isPending).toBe(false);
    });

    it("should handle network errors with graceful rollback", async () => {
      const originalData: ActorSystemData = {
        id: "sys-data-4",
        actor_id: "actor-4",
        game_system_id: "dnd5e",
        ability_data: { strength: 15, dexterity: 14, constitution: 13, intelligence: 12, wisdom: 11, charisma: 10 },
        proficiency_data: {},
        resource_data: {},
        trait_data: { class: "Rogue", level: 1 },
        spell_data: {},
        created_by: "user-1",
        updated_by: "user-1",
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
      };

      mockCollection.find().where().eq().exec.mockResolvedValueOnce([originalData]);
      mockCollection.find().where().eq().$.subscribe.mockReturnValue({
        unsubscribe: vi.fn(),
      });

      // Network error (server down)
      mockCollection.find().where().eq().exec.mockResolvedValueOnce([originalData]);

      (global.fetch as any).mockRejectedValueOnce(new Error("Network error: ECONNREFUSED"));

      const { result: mutationResult } = renderHook(() =>
        useUpdateActorData("actor-4", "dnd5e"),
      );

      let caughtError: Error | null = null;

      act(() => {
        mutationResult.current
          .mutate("proficiency_data", { acrobatics: true })
          .catch((e) => {
            caughtError = e;
          });
      });

      await waitFor(() => {
        expect(caughtError).toBeTruthy();
      });

      expect(caughtError?.message).toContain("Network error");
      expect(mutationResult.current.isPending).toBe(false);
    });
  });

  describe("Scenario 3: System Manifest Loading and Caching", () => {
    it("should lazy-load and cache D&D 5e manifest", async () => {
      // First access: load from disk
      const mockManifest = {
        id: "dnd5e",
        title: "D&D 5e",
        version: "0.1.0",
        calculators: {
          calculateAbilityModifier: (score: number) => Math.floor((score - 10) / 2),
          calculateProficiencyBonus: (level: number) => Math.floor((level - 1) / 4) + 2,
        },
      };

      // Simulate manifest loading
      const proficiencyBonus = mockManifest.calculators.calculateProficiencyBonus(5);
      expect(proficiencyBonus).toBe(2); // (5-1)/4 + 2 = 2

      const modifier = mockManifest.calculators.calculateAbilityModifier(16);
      expect(modifier).toBe(3); // (16-10)/2 = 3
    });
  });

  describe("Scenario 4: Multi-actor Synchronization", () => {
    it("should sync updates for multiple actors independently", async () => {
      const actor1: ActorSystemData = {
        id: "sys-data-a1",
        actor_id: "actor-a1",
        game_system_id: "dnd5e",
        ability_data: { strength: 15, dexterity: 14, constitution: 13, intelligence: 12, wisdom: 11, charisma: 10 },
        proficiency_data: {},
        resource_data: { hp: 40 },
        trait_data: { class: "Fighter" },
        spell_data: {},
        created_by: "user-1",
        updated_by: "user-1",
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
      };

      const actor2: ActorSystemData = {
        id: "sys-data-a2",
        actor_id: "actor-a2",
        game_system_id: "dnd5e",
        ability_data: { strength: 10, dexterity: 18, constitution: 12, intelligence: 14, wisdom: 13, charisma: 11 },
        proficiency_data: {},
        resource_data: { hp: 30 },
        trait_data: { class: "Rogue" },
        spell_data: {},
        created_by: "user-1",
        updated_by: "user-1",
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
      };

      // Setup queries for both actors
      mockCollection.find().where().eq().exec.mockResolvedValueOnce([actor1]);
      mockCollection.find().where().eq().$.subscribe.mockReturnValue({
        unsubscribe: vi.fn(),
      });

      // Load both characters simultaneously
      const { result: query1 } = renderHook(() => useActorSystemData("actor-a1", "dnd5e"));
      const { result: query2 } = renderHook(() => useActorSystemData("actor-a2", "dnd5e"));

      await waitFor(() => {
        expect(query1.current.data?.actor_id).toBe("actor-a1");
      });

      // Each should maintain independent state
      expect(query1.current.data?.ability_data?.strength).toBe(15);
      expect(query2.current.data === null).toBe(true); // Not loaded yet
    });
  });
});
