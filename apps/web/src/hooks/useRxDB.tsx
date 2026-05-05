import { useContext, createContext, ReactNode, useEffect, useState } from 'react';

/**
 * RxDB Context Hook
 * 
 * Phase 4.7.C2 integration point for RxDB token sync.
 * Currently provides a stub implementation that can be extended
 * with real RxDB when persistence layer is ready.
 */

export interface RxDBContextValue {
  db: {
    collections: {
      worldTokensCollection?: any;
      world_tokens?: any;
    };
  } | null;
  loading: boolean;
  error?: Error;
}

export const RxDBContext = createContext<RxDBContextValue | undefined>(undefined);

export function useRxDB(): RxDBContextValue {
  const context = useContext(RxDBContext);
  if (!context) {
    // Return a stub implementation if context not provided
    return {
      db: null,
      loading: false,
      error: undefined,
    };
  }
  return context;
}

/**
 * RxDB Provider Component
 * Wraps the app and provides database context to child components
 */
export function RxDBProvider({ children }: { children: ReactNode }) {
  const [value, setValue] = useState<RxDBContextValue>({
    db: null,
    loading: true,
  });

  useEffect(() => {
    // TODO: Initialize RxDB here when ready
    // For now, return stub implementation
    setValue({
      db: {
        collections: {
          worldTokensCollection: undefined,
          world_tokens: undefined,
        },
      },
      loading: false,
    });
  }, []);

  return (
    <RxDBContext.Provider value={value}>
      {children}
    </RxDBContext.Provider>
  );
}
