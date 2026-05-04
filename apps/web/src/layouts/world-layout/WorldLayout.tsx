import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import styles from "./WorldLayout.module.scss";

interface WorldLayoutProps {
  worldId: string;
  canvas: ReactNode;
  whiteboard: ReactNode;
}

export function WorldLayout({ worldId, canvas, whiteboard }: WorldLayoutProps) {
  return (
    <main className={styles.layout} data-world-id={worldId}>
      <header className={styles.header}>
        <div>
          <p className={styles.eyebrow}>World workspace</p>
          <h1>{worldId}</h1>
        </div>
        <Link to="/counter" className={styles.link}>
          Return to dashboard
        </Link>
      </header>

      <section className={styles.grid}>
        <div className={styles.canvas}>{canvas}</div>
        <aside className={styles.whiteboard}>{whiteboard}</aside>
      </section>
    </main>
  );
}
