/**
 * The system-hooks context, apart from the provider that fills it.
 *
 * Split out for two reasons. A module exporting both a component and a
 * non-component cannot fast-refresh, so an edit to the provider forced a full
 * reload; and `useSystemHooks` imported the context from the provider while
 * the provider imported the contract from `useSystemHooks`, a cycle that only
 * held together because the two halves were types and a `createContext` call.
 */
import { createContext } from "react";

import type { SystemHooksContract } from "../hooks/useSystemHooks";

/**
 * System context value
 */
export interface SystemContextValue {
  systemId?: string;
  hooks: SystemHooksContract;
  loading: boolean;
  error?: string;
  reload: () => Promise<void>;
}

export const SystemHooksContext = createContext<SystemContextValue | undefined>(
  undefined,
);
