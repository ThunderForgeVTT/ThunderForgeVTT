import {
  createElement,
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type PropsWithChildren,
} from "react";
import {
  getCurrentSession,
  login as loginRequest,
  logout as logoutRequest,
  refresh as refreshRequest,
  register as registerRequest,
  verifyTwoFactor as verifyTwoFactorRequest,
} from "@/api/auth";
import type {
  AuthSession,
  AuthSessionResponse,
  AuthUser,
  LoginPayload,
  RegisterPayload,
} from "@/types/auth";
import { clearAssetCache } from "@/serviceWorker";
import { discardWorldCache } from "@/services/worldCache";

type AuthContextValue = {
  user: AuthUser | null;
  session: AuthSession | null;
  isAuthenticated: boolean;
  isAdmin: boolean;
  isLoading: boolean;
  login: (payload: LoginPayload) => Promise<AuthSessionResponse>;
  completeTwoFactorChallenge: (
    challengeId: string,
    code: string,
  ) => Promise<AuthSessionResponse>;
  register: (payload: RegisterPayload) => Promise<AuthSessionResponse>;
  redirectAfterLogin: (userOverride?: AuthUser | null) => string;
  logout: () => Promise<void>;
  refresh: () => Promise<AuthSessionResponse | null>;
};

const AuthContext = createContext<AuthContextValue | null>(null);

function applySession(
  response: AuthSessionResponse,
  setSession: (value: AuthSession | null) => void,
) {
  setSession(response.session ?? null);
  return response;
}

export function AuthProvider({ children }: PropsWithChildren) {
  const [session, setSession] = useState<AuthSession | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const response = await refreshRequest();
      return applySession(response, setSession);
    } catch (error) {
      setSession(null);
      if (
        error instanceof Error &&
        error.message === "No active session to refresh"
      ) {
        return null;
      }
      throw error;
    }
  }, []);

  useEffect(() => {
    let active = true;

    void getCurrentSession()
      .then((response) => {
        if (active) {
          applySession(response, setSession);
        }
      })
      .catch(() => {
        if (active) {
          setSession(null);
        }
      })
      .finally(() => {
        if (active) {
          setIsLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, []);

  const login = useCallback(async (payload: LoginPayload) => {
    const response = await loginRequest(payload);
    return applySession(response, setSession);
  }, []);

  const completeTwoFactorChallenge = useCallback(
    async (challengeId: string, code: string) => {
      const verification = await verifyTwoFactorRequest(challengeId, code);
      const refreshed = await refreshRequest();
      setSession(refreshed.session ?? null);

      return {
        ...refreshed,
        status: verification.status,
        message: verification.message,
        loginTwoFactorChallengeId: null,
        requiresEmailVerification: refreshed.requiresEmailVerification,
      };
    },
    [],
  );

  const register = useCallback(async (payload: RegisterPayload) => {
    const response = await registerRequest(payload);
    return applySession(response, setSession);
  }, []);

  const redirectAfterLogin = useCallback(
    (userOverride?: AuthUser | null) => {
      const resolvedUser = userOverride ?? session?.user ?? null;
      return resolvedUser?.role === "admin" ? "/admin" : "/welcome";
    },
    [session],
  );

  const logout = useCallback(async () => {
    // Captured before the session is torn down: the engine derives the
    // signed-out user's cache directory from this id, and after `setSession`
    // there is nothing left to derive it from.
    const userId = session?.user?.id ?? null;

    await logoutRequest();
    setSession(null);
    // Cached scene art is per browser profile, not per account. Two people
    // sharing a machine would otherwise share it, and the second would read
    // bytes the server never authorised for them.
    clearAssetCache();
    // Same reason, for the encrypted world cache (spec 028, FR-016a): drop the
    // session key so everything it holds on disk is inert from here on,
    // whether or not the bytes ever finish being deleted. Awaited because the
    // discard is one IndexedDB delete, and never rejects — a cache that could
    // fail a sign-out would be worse than no cache.
    await discardWorldCache(userId);
  }, [session]);

  const value = useMemo<AuthContextValue>(
    () => ({
      user: session?.user ?? null,
      session,
      isAuthenticated: Boolean(session?.authenticated),
      isAdmin: session?.user?.role === "admin",
      isLoading,
      login,
      completeTwoFactorChallenge,
      register,
      redirectAfterLogin,
      logout,
      refresh,
    }),
    [
      completeTwoFactorChallenge,
      isLoading,
      login,
      logout,
      redirectAfterLogin,
      refresh,
      register,
      session,
    ],
  );

  return createElement(AuthContext.Provider, { value }, children);
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error("useAuth must be used inside AuthProvider");
  }

  return context;
}
