import { Button } from "@/components/ui/button";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { useTheme } from "@/hooks/theme-context";

export function ThemeToggle() {
  const { theme, toggleTheme } = useTheme();
  const isDark = theme === "dark";

  return (
    <Button
      variant="ghost"
      size="icon"
      aria-label={isDark ? "Switch to light mode" : "Switch to dark mode"}
      onClick={toggleTheme}
    >
      <FantasyIcon name={isDark ? "sun" : "moon"} size={16} />
    </Button>
  );
}
