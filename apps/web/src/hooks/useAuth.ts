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
} from "@/api/auth";
import type {
  AuthSession,
  AuthSessionResponse,
  AuthUser,
  LoginPayload,
  RegisterPayload,
} from "@/types/auth";

type AuthContextValue = {
  user: AuthUser | null;
  session: AuthSession | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  login: (payload: LoginPayload) => Promise<AuthSessionResponse>;
  register: (payload: RegisterPayload) => Promise<AuthSessionResponse>;
  logout: () => Promise<void>;
  refresh: () => Promise<AuthSessionResponse | null>;
};

const AuthContext = createContext<AuthContextValue | null>(null);

function applySession(response: AuthSessionResponse, setSession: (value: AuthSession | null) => void) {
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
      if (error instanceof Error && error.message === "No active session to refresh") {
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

  const register = useCallback(async (payload: RegisterPayload) => {
    const response = await registerRequest(payload);
    return applySession(response, setSession);
  }, []);

  const logout = useCallback(async () => {
    await logoutRequest();
    setSession(null);
  }, []);

  const value = useMemo<AuthContextValue>(
    () => ({
      user: session?.user ?? null,
      session,
      isAuthenticated: Boolean(session?.authenticated),
      isLoading,
      login,
      register,
      logout,
      refresh,
    }),
    [isLoading, login, logout, refresh, register, session],
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
