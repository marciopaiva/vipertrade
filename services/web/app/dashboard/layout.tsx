import { AppHeader } from '@/components/AppHeader';

export default async function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  // Middleware already ensures session exists; no need to check again
  return (
    <div className="min-h-screen bg-viper-navy">
      <AppHeader />
      {/* Main Content */}
      <main className="container mx-auto px-4 py-6">{children}</main>
    </div>
  );
}
