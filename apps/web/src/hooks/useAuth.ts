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
import { onSessionExpired, resetSessionExpiry } from "@/api/sessionExpiry";
import { discardWorldCache, onCrossTabSignOut } from "@/services/worldCache";

type AuthContextValue = {
  user: AuthUser | null;
  session: AuthSession | null;
  isAuthenticated: boolean;
  isAdmin: boolean;
  /** Spec 039 US7: disabled — the standing page is the only place to go. */
  isDisabled: boolean;
  isLoading: boolean;
  login: (payload: LoginPayload) => Promise<AuthSessionResponse>;
  /** Exactly one of `code` or `recoveryCode` — see `verifyTwoFactor`. */
  completeTwoFactorChallenge: (
    challengeId: string,
    credential: { code?: string; recoveryCode?: string },
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
  // Re-arm the FR-009 announcement whenever a session is established, so
  // signing back in after a revocation is a fresh start rather than a client
  // that has already spent its one notification and will never say it again.
  if (response.session) {
    resetSessionExpiry();
  }
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

  // FR-021e: signing out in one tab signs the user out of every tab.
  //
  // The session cookie is shared, so those tabs *are* already signed out —
  // the server will refuse their next request. But until something happens to
  // fail, they keep presenting a signed-in application: a world still on
  // screen, a character sheet, a chat log. That is content the person
  // believes they have closed, which is the same concern the cache key
  // addresses, one layer up.
  //
  // Deliberately clears local state only. It does NOT call `logout()`: the
  // originating tab already hit the endpoint and cleared the cookie, and
  // having every other tab race to do it again would mean N redundant
  // requests and an error in each of the ones that lose.
  useEffect(() => {
    return onCrossTabSignOut(() => {
      setSession(null);
      setIsLoading(false);
    });
  }, []);

  /**
   * Spec 036 FR-009: told to sign in again, rather than left showing state it
   * can no longer refresh.
   *
   * The transport reports any 401 (`api/sessionExpiry`), and this is what acts
   * on it: drop the session so every guarded route redirects, and discard the
   * world cache, because a session that is no longer accepted must not leave
   * decrypted world content readable on this device.
   *
   * `discardWorldCache` broadcasts the cross-tab sign-out signal too, so the
   * other tabs of a revoked session find out without each having to make a
   * failing request of their own.
   */
  useEffect(() => {
    // The id is read before the session is dropped: `discardWorldCache` needs
    // it to reclaim the engine's store, and a moment later there is nobody to
    // ask. Not awaited — the key is gone the instant `discardSessionKeys`
    // resolves, which is the part that makes the cached bytes inert, and a
    // signed-out person should not wait on a delete.
    return onSessionExpired(() => {
      const userId = session?.user?.id ?? null;
      setSession(null);
      setIsLoading(false);
      void discardWorldCache(userId);
    });
  }, [session]);

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
    async (
      challengeId: string,
      credential: { code?: string; recoveryCode?: string },
    ) => {
      const verification = await verifyTwoFactorRequest(
        challengeId,
        credential,
      );
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
      isDisabled: Boolean(session?.accountDisabled),
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
