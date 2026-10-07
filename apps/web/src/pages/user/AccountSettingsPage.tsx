import { useState } from "react";
import { deleteUserData, exportUserData } from "@/api/auth";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Dialog } from "@/components/ui/dialog/Dialog";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import { IN_DEMO } from "@/lib/demoBuild";
import type { SeoConfig } from "@/types/seo";

export const accountSettingsPageSeo: SeoConfig = {
  title: "Your account",
  description: "Download what this account holds, delete it, or sign out.",
  canonicalPath: "/settings/account",
  noindex: true,
};

/**
 * Spec 074 FR-001: the account's own controls, under the account.
 *
 * Export and deletion used to live on `/counter`, a component gallery nobody
 * would look on for them. They are a person's rights over their own data, so
 * they sit beside security, storage and standing, where an account's other
 * settings already are.
 */
export function AccountSettingsPage() {
  const { logout, user } = useAuth();
  const [status, setStatus] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState<"json" | "zip" | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);

  const downloadExport = async (format: "json" | "zip") => {
    setIsExporting(format);
    setStatus(null);

    try {
      const blob = await exportUserData(format);
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download =
        format === "zip"
          ? "thunderforge-user-export.zip"
          : "thunderforge-user-export.json";
      anchor.click();
      URL.revokeObjectURL(url);
      setStatus(
        format === "zip"
          ? "ZIP export is ready for download."
          : "JSON export is ready for download.",
      );
    } catch (error) {
      setStatus(
        error instanceof Error ? error.message : "Export request failed.",
      );
    } finally {
      setIsExporting(null);
    }
  };

  const permanentlyDeleteAccount = async () => {
    setIsDeleting(true);
    setStatus(null);

    try {
      const response = await deleteUserData();
      setStatus(response.message);
      await logout();
      window.location.assign("/login");
    } catch (error) {
      setStatus(
        error instanceof Error ? error.message : "Delete request failed.",
      );
      setIsDeleting(false);
    }
  };

  const signOut = async () => {
    await logout();
    window.location.assign("/login");
  };

  return (
    <>
      <SEO {...accountSettingsPageSeo} />
      <Container>
        <div className="grid gap-6 py-8" data-testid="account-settings">
          <header className="grid gap-1">
            <h1 className="text-2xl font-semibold">Your account</h1>
            <p className="text-sm text-muted-foreground">
              Signed in as <strong>{user?.username ?? "this session"}</strong>.
            </p>
          </header>

          {/* Spec 074 FR-013: the demo visitor has no account to export, end
              or delete; what the demo holds stays in this browser. */}
          {IN_DEMO ? (
            <Card
              surface="stone"
              className="grid gap-1 p-6"
              data-testid="account-demo"
            >
              <h2 className="text-xl font-semibold">A demo visitor</h2>
              <p className="text-muted-foreground">
                The demo keeps what you make in this browser, not in an account.
                A ThunderForge instance lets you export your data, sign out and
                delete your account from here.
              </p>
            </Card>
          ) : (
            <>
              <Card surface="stone" className="grid gap-4 p-6">
                <div className="grid gap-1">
                  <h2 className="text-xl font-semibold">Your data</h2>
                  <p className="text-muted-foreground">
                    Everything this account holds here, as a file you keep.
                  </p>
                </div>
                <div className="flex flex-wrap gap-3">
                  <Button
                    variant="secondary"
                    icon="actors"
                    disabled={isExporting !== null}
                    onClick={() => void downloadExport("json")}
                  >
                    {isExporting === "json"
                      ? "Preparing JSON..."
                      : "Download JSON export"}
                  </Button>
                  <Button
                    variant="secondary"
                    icon="inventory"
                    disabled={isExporting !== null}
                    onClick={() => void downloadExport("zip")}
                  >
                    {isExporting === "zip"
                      ? "Preparing ZIP..."
                      : "Download ZIP export"}
                  </Button>
                </div>
              </Card>

              <Card surface="stone" className="grid gap-4 p-6">
                <div className="grid gap-1">
                  <h2 className="text-xl font-semibold">Sign out</h2>
                  <p className="text-muted-foreground">
                    Ends this session on this device. Your account and
                    everything in it stay as they are.
                  </p>
                </div>
                <div>
                  <Button
                    variant="secondary"
                    icon="arrow-left"
                    onClick={() => void signOut()}
                  >
                    Sign out
                  </Button>
                </div>
              </Card>

              <Card surface="stone" className="grid gap-4 p-6">
                <div className="grid gap-1">
                  <h2 className="text-xl font-semibold">Delete this account</h2>
                  <p className="text-muted-foreground">
                    Permanent. Download an export first if you want to keep
                    anything.
                  </p>
                </div>
                <div>
                  <Dialog
                    trigger={
                      <Button
                        variant="danger"
                        icon="skull"
                        disabled={isDeleting}
                      >
                        {isDeleting ? "Deleting account..." : "Delete account"}
                      </Button>
                    }
                    title="Permanently delete this account?"
                    description="This deletes your profile, sign-ins and sessions, every world you created, and your whole library: every book you read in and every collection you wrote. Players in your worlds keep their characters, which are copied to their own accounts first."
                    footer={
                      <Button
                        variant="danger"
                        icon="skull"
                        disabled={isDeleting}
                        onClick={() => void permanentlyDeleteAccount()}
                      >
                        {isDeleting ? "Deleting..." : "Delete permanently"}
                      </Button>
                    }
                  >
                    {/* Spec 050 FR-062 to FR-064: what is true, not what is
                    reassuring. There is no shared store behind the library, so
                    nothing of it is kept anywhere once this is done. */}
                    <p data-testid="delete-account-consequence">
                      This cannot be undone. Nothing of your library is kept
                      anywhere afterwards: there is no shared copy of any book
                      to keep.
                    </p>
                  </Dialog>
                </div>
              </Card>
            </>
          )}

          {status ? <StatusBadge>{status}</StatusBadge> : null}
        </div>
      </Container>
    </>
  );
}
export default AccountSettingsPage;
