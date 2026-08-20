import type { FormEvent } from "react";
import { useEffect, useState } from "react";
import { Link, useNavigate, useParams, useSearchParams } from "react-router-dom";
import {
  confirmOAuthLink,
  consumeOAuthReturnTo,
  exchangeOAuthCode,
  verifyTwoFactor,
} from "@/api/auth";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import { AuthLayout } from "@/layouts/auth-layout/AuthLayout";
import type { OAuthActionResponse } from "@/types/auth";
import type { SeoConfig } from "@/types/seo";

export const oauthCallbackPageSeo: SeoConfig = {
  title: "OAuth callback",
  description: "Complete OAuth sign-in or account linking for ThunderForge VTT.",
  canonicalPath: "/oauth/callback",
  noindex: true,
};

function extractOAuthPayload(error: unknown): OAuthActionResponse | null {
  if (
    error instanceof Error &&
    "response" in error &&
    typeof error.response === "object" &&
    error.response !== null
  ) {
    return error.response as OAuthActionResponse;
  }

  return null;
}

export default function OAuthCallbackPage() {
  const navigate = useNavigate();
  const { providerKey = "" } = useParams();
  const [searchParams] = useSearchParams();
  const code = searchParams.get("code");
  const state = searchParams.get("state");
  const providerError = searchParams.get("error");
  const providerErrorDescription = searchParams.get("error_description");
  const callbackError = providerError
    ? providerErrorDescription
      ? `${providerError}: ${providerErrorDescription}`
      : providerError
    : !providerKey || !code || !state
      ? "OAuth callback is missing the provider, code, or state."
      : null;
  const { refresh } = useAuth();
  const [status, setStatus] = useState<string | null>(callbackError);
  const [isWorking, setIsWorking] = useState(callbackError === null);
  const [linkChallengeId, setLinkChallengeId] = useState<string | null>(null);
  const [twoFactorChallengeId, setTwoFactorChallengeId] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [twoFactorCode, setTwoFactorCode] = useState("");

  useEffect(() => {
    if (callbackError || !code || !state || !providerKey) {
      return;
    }

    let active = true;

    const completeExchange = async () => {
      try {
        const response = await exchangeOAuthCode(providerKey, code, state);
        if (!active) {
          return;
        }

        setStatus(response.message);
        await refresh();
        navigate(consumeOAuthReturnTo("/welcome"), { replace: true });
      } catch (error) {
        if (!active) {
          return;
        }

        const payload = extractOAuthPayload(error);
        if (payload?.status === "password_required") {
          setLinkChallengeId(payload.challengeId);
          setStatus(payload.message);
        } else if (payload?.status === "two_factor_required") {
          setTwoFactorChallengeId(payload.loginTwoFactorChallengeId);
          setStatus(payload.message);
        } else {
          setStatus(error instanceof Error ? error.message : "OAuth sign-in failed.");
        }
      } finally {
        if (active) {
          setIsWorking(false);
        }
      }
    };

    void completeExchange();

    return () => {
      active = false;
    };
  }, [callbackError, code, navigate, providerKey, refresh, state]);

  const onConfirmLink = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!linkChallengeId) {
      return;
    }

    setIsWorking(true);
    try {
      const response = await confirmOAuthLink(linkChallengeId, password);
      setStatus(response.message);
      await refresh();
      navigate(consumeOAuthReturnTo("/welcome"), { replace: true });
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Failed to confirm account link.");
    } finally {
      setIsWorking(false);
    }
  };

  const onVerifyTwoFactor = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!twoFactorChallengeId) {
      return;
    }

    setIsWorking(true);
    try {
      const response = await verifyTwoFactor(twoFactorChallengeId, twoFactorCode);
      setStatus(response.message);
      await refresh();
      navigate(consumeOAuthReturnTo("/welcome"), { replace: true });
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Failed to verify 2FA.");
    } finally {
      setIsWorking(false);
    }
  };

  return (
    <>
      <SEO {...oauthCallbackPageSeo} />
      <AuthLayout
        eyebrow="OAuth"
        title="Completing your sign-in ritual."
        description="ThunderForge exchanges the provider code server-side, then either restores your session, asks for local account confirmation, or requires a 2FA proof."
      >
        <Card>
          <div className="grid gap-4">
            <div className="grid gap-1.5">
              <h2 className="text-lg font-semibold">
                {providerKey ? `Provider: ${providerKey}` : "OAuth callback"}
              </h2>
              <p className="text-muted-foreground">
                Keep this page open while the callback is processed.
              </p>
            </div>

            {status ? <StatusBadge>{status}</StatusBadge> : null}

            {isWorking ? (
              <p className="text-muted-foreground">
                Resolving provider identity and restoring your ThunderForge
                session...
              </p>
            ) : null}

            {linkChallengeId ? (
              <form onSubmit={onConfirmLink} className="grid gap-4">
                <RuneDivider label="Link existing account" />
                <Field
                  label="Local account password"
                  htmlFor="oauth-link-password"
                  hint="ThunderForge requires explicit confirmation before linking a provider to an existing local account."
                >
                  <Input
                    id="oauth-link-password"
                    type="password"
                    autoComplete="current-password"
                    value={password}
                    onChange={(event) => setPassword(event.target.value)}
                    placeholder="Enter your local account password"
                  />
                </Field>
                <Button type="submit" icon="shield" disabled={isWorking}>
                  {isWorking ? "Linking account..." : "Confirm link"}
                </Button>
              </form>
            ) : null}

            {twoFactorChallengeId ? (
              <form onSubmit={onVerifyTwoFactor} className="grid gap-4">
                <RuneDivider label="Two-factor verification" />
                <Field label="2FA code" htmlFor="oauth-two-factor-code">
                  <Input
                    id="oauth-two-factor-code"
                    inputMode="numeric"
                    value={twoFactorCode}
                    onChange={(event) => setTwoFactorCode(event.target.value)}
                    placeholder="123456"
                  />
                </Field>
                <Button type="submit" icon="spark" disabled={isWorking}>
                  {isWorking ? "Verifying..." : "Verify 2FA"}
                </Button>
              </form>
            ) : null}

            <div className="grid gap-2">
              <Link to="/login" className="font-medium text-primary hover:underline">
                Return to login
              </Link>
              <Link to="/register" className="font-medium text-primary hover:underline">
                Create a local account
              </Link>
            </div>
          </div>
        </Card>
      </AuthLayout>
    </>
  );
}
