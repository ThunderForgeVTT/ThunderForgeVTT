import type { ReactNode } from "react";
import { Avatar } from "@/components/ui/avatar/Avatar";
import { Container } from "@/components/ui/container/Container";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";
import styles from "./AuthLayout.module.scss";

interface AuthLayoutProps {
  eyebrow: string;
  title: string;
  description: string;
  children: ReactNode;
  aside?: ReactNode;
}

export function AuthLayout({
  eyebrow,
  title,
  description,
  children,
  aside,
}: AuthLayoutProps) {
  return (
    <main className={styles.shell}>
      <Container className={styles.container}>
        <section className={styles.hero}>
          <p className={styles.eyebrow}>{eyebrow}</p>
          <h1>{title}</h1>
          <p>{description}</p>
          <div className={styles.party}>
            <div className={styles.avatarRow}>
              <Avatar seed="scribe" name="Scribe" size="sm" />
              <Avatar seed="warden" name="Warden" size="sm" />
              <Avatar seed="seer" name="Seer" size="sm" />
            </div>
            <span>
              Guild access, onboarding, and realm stewardship all route through
              the same typed shell.
            </span>
          </div>
        </section>

        <RuneDivider label="Access rituals" />

        <div className={styles.grid}>
          <div className={styles.primary}>{children}</div>
          {aside ? <aside className={styles.aside}>{aside}</aside> : null}
        </div>
      </Container>
    </main>
  );
}
