import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getSetupStatus } from "@/api/auth";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import type { SetupProvider } from "@/types/auth";

/**
 * Spec 035 (FR-016): `/invite/:code` — where an instance invitation is
 * redeemed.
 *
 * # This page renders for a signed-out visitor
 *
 * That is the entire point, and its route is deliberately outside
 * `RequireAuthenticated`. A redemption page behind a login wall admits nobody:
 * the person holding this link has no account yet, which is what the link is
 * for. The three shared-content pages made exactly this mistake before
 * ADR-071, and the fix there was the same one.
 *
 * The code is carried to whichever route the visitor picks — appended to the
 * registration form's submission, or to the provider's `start` call, where it
 * rides the authorization session across the redirect.
 */
export default function InstanceInvitePage() {
  const { code = "" } = useParams();
  const navigate = useNavigate();
  const [providers, setProviders] = useState<SetupProvider[]>([]);

  useEffect(() => {
    let active = true;
    void getSetupStatus()
      .then((status) => {
        if (active) {
          setProviders(status.configured_oauth_providers);
        }
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, []);

  return (
    <>
      <SEO
        title="You have been invited"
        description="Redeem an invitation to this ThunderForge instance"
        noindex
      />
      <Container>
        <main className="grid min-h-[60vh] place-items-center py-16">
          <Card className="grid w-full max-w-lg gap-4 p-6">
            <div>
              <h1 className="text-2xl font-semibold">You have been invited</h1>
              <p className="text-muted-foreground">
                This instance is not open to the public. Create your account
                below and the invitation will be redeemed as you go.
              </p>
            </div>

            <Button
              onClick={() =>
                navigate(`/register?invitation=${encodeURIComponent(code)}`)
              }
              data-testid="invite-register"
            >
              Create an account
            </Button>

            {providers.length > 0 ? (
              <div className="grid gap-2">
                <p className="text-sm text-muted-foreground">
                  Or continue with a provider:
                </p>
                {providers.map((provider) => (
                  <Button
                    key={provider.provider_key}
                    variant="secondary"
                    data-testid={`invite-provider-${provider.provider_key}`}
                    onClick={() => {
                      // The code rides the authorization session the flow
                      // already creates, so it survives the provider redirect
                      // without a cookie of its own.
                      const redirectUri = `${window.location.origin}/auth/callback`;
                      window.location.href =
                        `/api/authentication/oauth/${provider.provider_key}/start` +
                        `?redirect_uri=${encodeURIComponent(redirectUri)}` +
                        `&invitation=${encodeURIComponent(code)}`;
                    }}
                  >
                    Continue with {provider.display_name}
                  </Button>
                ))}
              </div>
            ) : null}
          </Card>
        </main>
      </Container>
    </>
  );
}
