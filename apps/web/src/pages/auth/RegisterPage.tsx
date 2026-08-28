import type { FormEvent } from "react";
import { useEffect, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { getSetupStatus, startOAuthLogin } from "@/api/auth";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { AuthLayout } from "@/layouts/auth-layout/AuthLayout";
import { useAuth } from "@/hooks/useAuth";
import type { SetupProvider } from "@/types/auth";
import type { SeoConfig } from "@/types/seo";

export const registerPageSeo: SeoConfig = {
  title: "Create account",
  description:
    "Create a local ThunderForge VTT account with secure password storage and session-backed authentication.",
  keywords: [
    "ThunderForge register",
    "create tabletop account",
    "virtual tabletop sign up",
  ],
  canonicalPath: "/register",
  prefetchHrefs: ["/login"],
};

function redirectTarget(search: string) {
  const params = new URLSearchParams(search);
  const returnTo = params.get("returnTo");
  return returnTo && returnTo.startsWith("/") ? returnTo : null;
}

export default function RegisterPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { register, redirectAfterLogin } = useAuth();
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [passwordConfirmation, setPasswordConfirmation] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [providers, setProviders] = useState<SetupProvider[]>([]);

  useEffect(() => {
    let active = true;

    void getSetupStatus()
      .then((response) => {
        if (active) {
          setProviders(response.configured_oauth_providers);
        }
      })
      .catch(() => {
        if (active) {
          setProviders([]);
        }
      });

    return () => {
      active = false;
    };
  }, []);

  const onSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    if (password !== passwordConfirmation) {
      setStatus("Passwords do not match.");
      return;
    }

    setIsSubmitting(true);
    setStatus(null);

    try {
      const response = await register({
        username,
        email,
        password,
      });
      setStatus(response.message);

      if (response.session?.authenticated) {
        navigate(
          redirectTarget(location.search) ??
            redirectAfterLogin(response.session.user),
          { replace: true },
        );
      }
    } catch (error) {
      setStatus(
        error instanceof Error ? error.message : "Registration failed.",
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <>
      <SEO {...registerPageSeo} />
      <AuthLayout
        aside={
          <Card className="p-5">
            <div className="grid gap-2">
              <Link
                to="/login"
                className="font-medium text-primary hover:underline"
              >
                Return to login
              </Link>
              <Link
                to="/welcome"
                className="font-medium text-primary hover:underline"
              >
                Open the welcome hall
              </Link>
            </div>
          </Card>
        }
      >
        <Card className="p-6">
          <form onSubmit={onSubmit} className="grid gap-4">
            <h2 className="text-lg font-semibold">Create account</h2>

            <Field label="Username" htmlFor="register-username">
              <Input
                id="register-username"
                name="username"
                autoComplete="username"
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                placeholder="world-builder"
              />
            </Field>

            <Field label="Email address" htmlFor="register-email">
              <Input
                id="register-email"
                name="email"
                type="email"
                autoComplete="email"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                placeholder="builder@thunderforge.app"
              />
            </Field>

            <Field
              label="Password"
              htmlFor="register-password"
              hint="Use at least 12 characters."
            >
              <Input
                id="register-password"
                name="password"
                type="password"
                autoComplete="new-password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                placeholder="Create a password"
              />
            </Field>

            <Field
              label="Confirm password"
              htmlFor="register-password-confirmation"
            >
              <Input
                id="register-password-confirmation"
                name="passwordConfirmation"
                type="password"
                autoComplete="new-password"
                value={passwordConfirmation}
                onChange={(event) =>
                  setPasswordConfirmation(event.target.value)
                }
                placeholder="Confirm your password"
              />
            </Field>

            <div className="flex flex-wrap gap-3">
              <Button
                type="submit"
                variant="success"
                size="lg"
                disabled={isSubmitting}
                icon="quill"
              >
                {isSubmitting ? "Creating account..." : "Create account"}
              </Button>
            </div>

            {status ? <StatusBadge>{status}</StatusBadge> : null}

            {providers.length ? (
              <>
                <RuneDivider label="Existing linked provider" />
                <div className="flex flex-wrap gap-3">
                  {providers.map((provider) => (
                    <Button
                      key={provider.provider_key}
                      type="button"
                      variant="secondary"
                      icon="spark"
                      onClick={() =>
                        startOAuthLogin(
                          provider.provider_key,
                          redirectTarget(location.search) ?? "/welcome",
                        )
                      }
                    >
                      Sign in with {provider.display_name}
                    </Button>
                  ))}
                </div>
              </>
            ) : null}
          </form>
        </Card>
      </AuthLayout>
    </>
  );
}
