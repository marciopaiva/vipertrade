import { AppHeader } from '@/components/AppHeader';
import { CommandPalette } from '@/components/console/CommandPalette';

/**
 * Shared shell for the operator console (dashboard, analysis, and the screens
 * that follow). One header + content frame for every console route, so pages
 * own only their content and can't drift apart. Routes keep their URLs — the
 * `(console)` group is structural and does not appear in the path.
 */
export default function ConsoleLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  // Auth (when enabled) is enforced by middleware before we get here.
  return (
    <div className="hud-grid min-h-screen bg-background">
      <AppHeader />
      <main className="container mx-auto px-4 py-6">{children}</main>
      <CommandPalette />
    </div>
  );
}
