/**
 * System Hooks API
 *
 * This module defines the contract for game-system-specific hooks.
 * Systems implement these interfaces to customize VTT behavior.
 */

import { useContext, useState, useEffect } from "react";
import { SystemHooksContext } from "../providers/system-hooks-context";

/**
 * Base token data (sent from server)
 */
export interface BaseTokenStats {
  health: number;
  maxHealth: number;
  strength: number;
  dexterity: number;
  constitution: number;
  intelligence: number;
  wisdom: number;
  charisma: number;
  [key: string]: number | string;
}

/**
 * Derived token stats (computed client-side by system hooks)
 */
export interface DerivedTokenStats {
  armorClass?: number;
  initiative?: number;
  healthPercentage?: number;
  isDead?: boolean;
  isFullHealth?: boolean;
  [key: string]: number | boolean | string | undefined;
}

/**
 * Token movement validation parameters
 */
export interface TokenMoveParams {
  tokenId: string;
  x: number;
  y: number;
  sceneId: string;
  currentX: number;
  currentY: number;
}

/**
 * Dice roll validation/formatting parameters
 */
export interface DiceRollParams {
  diceStr: string; // "4d6", "2d20kh1", etc.
  modifier?: number;
}

/**
 * Parsed dice roll result
 */
export interface DiceRollResult {
  valid: boolean;
  error?: string;
  dice: string;
  modifier?: number;
  total?: number;
  rolls?: number[];
}

/**
 * Damage formatting parameters
 */
export interface DamageFormatParams {
  diceStr: string; // "2d6+3"
}

/**
 * Condition change parameters
 */
export interface ConditionChangeParams {
  tokenId: string;
  condition: string; // "poisoned", "paralyzed", etc.
  applied: boolean;
}

/**
 * Token visibility check parameters
 */
export interface TokenVisibilityParams {
  fromTokenId: string;
  toTokenId: string;
  fogMask?: string; // Base64 or URL to fog bitmap
}

/**
 * System Hook Contract - all available hooks a system can implement
 */
export interface SystemHooksContract {
  /**
   * Called when a token is about to move.
   * Return false to reject the move, true to allow it.
   */
  onTokenMove?: (params: TokenMoveParams) => boolean | Promise<boolean>;

  /**
   * Called to compute derived token statistics from base stats.
   * Should return computed derived stats (never sent over network).
   */
  computeDerivedStats?: (
    baseStats: BaseTokenStats,
  ) => DerivedTokenStats | Promise<DerivedTokenStats>;

  /**
   * Called to validate a dice roll string.
   * Returns parsed roll or error details.
   */
  validateRoll?: (
    params: DiceRollParams,
  ) => DiceRollResult | Promise<DiceRollResult>;

  /**
   * Called to format damage output for display.
   * Example: "2d6+3" -> "2d6+3 (avg: 10)"
   */
  formatDamage?: (params: DamageFormatParams) => string | Promise<string>;

  /**
   * Called when a condition is applied or removed.
   * Return false to prevent the change, true to allow it.
   */
  onConditionChange?: (
    params: ConditionChangeParams,
  ) => boolean | Promise<boolean>;

  /**
   * Called to check if one token can see another (fog of war).
   * Return true if visible, false if hidden.
   */
  checkTokenVisibility?: (
    params: TokenVisibilityParams,
  ) => boolean | Promise<boolean>;

  /**
   * Called to compute armor class for a token.
   * Receives base stats, returns AC value.
   */
  computeArmorClass?: (baseStats: BaseTokenStats) => number | Promise<number>;
}

/**
 * Hook invocation status
 */
export interface HookInvokeStatus {
  loading: boolean;
  error?: string;
  /**
   * Whatever the invoked hook returned. Each hook in the contract returns
   * something different and `useSystemHook` is generic over all of them, so
   * callers narrow this to the return type of the hook they asked for.
   */
  result?: unknown;
}

/**
 * useSystemHooks hook
 * Access system hooks in any React component
 */
export function useSystemHooks() {
  const context = useContext(SystemHooksContext);
  if (!context) {
    throw new Error("useSystemHooks must be used within SystemHooksProvider");
  }
  return context;
}

/**
 * Helper hook to invoke a system hook and track loading/error state
 */
export function useSystemHook<T extends keyof SystemHooksContract>(
  hookName: T,
  params?: Parameters<NonNullable<SystemHooksContract[T]>>[0],
): HookInvokeStatus {
  const { hooks } = useSystemHooks();
  const [status, setStatus] = useState<HookInvokeStatus>({ loading: false });

  useEffect(() => {
    if (!params) return;

    const invokeHook = async () => {
      setStatus({ loading: true });
      try {
        // Once `T` is generic, `hooks[hookName]` is a union of every hook
        // signature, and TypeScript refuses to call such a union even though
        // `params` is already typed as that same hook's parameter. This
        // restates the relationship the signature guarantees rather than
        // widening anything.
        const hookFn = hooks[hookName] as
          | ((p: Parameters<NonNullable<SystemHooksContract[T]>>[0]) => unknown)
          | undefined;
        if (!hookFn) {
          setStatus({
            loading: false,
            error: `Hook ${hookName} not implemented by system`,
          });
          return;
        }

        const result = await hookFn(params);
        setStatus({ loading: false, result });
      } catch (error) {
        setStatus({
          loading: false,
          error: error instanceof Error ? error.message : "Unknown error",
        });
      }
    };

    invokeHook();
  }, [hooks, hookName, params]);

  return status;
}

/**
 * Helper to compute derived stats for a token
 * Returns base stats if system has no hook
 */
export async function computeTokenDerivedStats(
  baseStats: BaseTokenStats,
  hooks?: SystemHooksContract,
): Promise<DerivedTokenStats> {
  if (!hooks?.computeDerivedStats) {
    // Default computation
    return {
      healthPercentage: (baseStats.health / baseStats.maxHealth) * 100,
      isDead: baseStats.health <= 0,
      isFullHealth: baseStats.health >= baseStats.maxHealth,
    };
  }

  return hooks.computeDerivedStats(baseStats);
}

/**
 * Helper to validate token movement
 * Returns true (allow) if system has no hook
 */
export async function validateTokenMove(
  params: TokenMoveParams,
  hooks?: SystemHooksContract,
): Promise<boolean> {
  if (!hooks?.onTokenMove) {
    return true;
  }

  return hooks.onTokenMove(params);
}

/**
 * Helper to format dice rolls
 * Returns input string if system has no hook
 */
export async function formatDiceRoll(
  diceStr: string,
  hooks?: SystemHooksContract,
): Promise<string> {
  if (!hooks?.formatDamage) {
    return diceStr;
  }

  return hooks.formatDamage({ diceStr });
}
