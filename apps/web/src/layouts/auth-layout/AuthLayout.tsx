import type { ReactNode } from "react";
import { Container } from "@/components/ui/container/Container";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";

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
    <main className="min-h-full py-12 pb-16">
      <Container className="grid gap-8">
        <section className="grid max-w-3xl gap-3">
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            {eyebrow}
          </p>
          <h1 className="text-4xl leading-none font-semibold text-balance sm:text-6xl">
            {title}
          </h1>
          <p className="max-w-2xl text-muted-foreground">{description}</p>
        </section>

        <RuneDivider label="Access rituals" />

        <div className="grid gap-6 md:grid-cols-[minmax(0,1.4fr)_minmax(0,0.9fr)]">
          <div className="min-w-0">{children}</div>
          {aside ? (
            <aside className="grid min-w-0 content-start gap-4">
              {aside}
            </aside>
          ) : null}
        </div>
      </Container>
    </main>
  );
}
