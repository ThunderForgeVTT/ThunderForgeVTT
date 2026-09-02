/**
 * The theme context and its reader, apart from the provider that fills it.
 *
 * Split out because a module that exports both a component and a hook cannot
 * fast-refresh: an edit to either forces a full reload, losing the state of
 * whatever was on screen. `ThemeProvider` stays in `useTheme.tsx`; the
 * context and `useTheme` live here, where nothing renders.
 */
import { createContext, useContext } from "react";

export type Theme = "light" | "dark";

export interface ThemeContextValue {
  theme: Theme;
  toggleTheme: () => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);

export function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useTheme must be used within a ThemeProvider");
  }
  return context;
}
