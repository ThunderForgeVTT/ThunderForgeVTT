import { cn } from "@/utils/cn";
import { useAvatar } from "@/hooks/useAvatar";
import styles from "./TokenAvatar.module.scss";

export interface TokenAvatarProps {
  seed: string;
  label?: string;
  className?: string;
}

export function TokenAvatar({ seed, label, className }: TokenAvatarProps) {
  const { tokenPngUrl } = useAvatar(seed);

  return (
    <div className={cn(styles.token, className)}>
      <img src={tokenPngUrl} alt={label ?? seed} className={styles.image} />
      {label ? <span className={styles.label}>{label}</span> : null}
    </div>
  );
}
