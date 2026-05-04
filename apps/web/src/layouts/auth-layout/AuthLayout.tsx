import type { ReactNode } from "react";
import { Container } from "@/components/ui/container/Container";
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
        </section>

        <div className={styles.grid}>
          <div className={styles.primary}>{children}</div>
          {aside ? <aside className={styles.aside}>{aside}</aside> : null}
        </div>
      </Container>
    </main>
  );
}
