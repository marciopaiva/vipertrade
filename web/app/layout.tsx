import "./globals.css";

export const metadata = {
  title: "ViperTrade Dashboard",
  description: "Operational dashboard for ViperTrade backend services",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
