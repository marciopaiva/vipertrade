'use client';

import { useRouter } from 'next/navigation';
import { useState } from 'react';

export default function LogoutButton() {
  const router = useRouter();
  const [loggingOut, setLoggingOut] = useState(false);

  const handleLogout = async () => {
    setLoggingOut(true);
    try {
      const response = await fetch('/api/logout', {
        method: 'POST',
        credentials: 'include',
        cache: 'no-store',
        redirect: 'manual'
      });

      // Remove cookie no cliente (backup)
      document.cookie = 'vipertrade_session=; Path=/; Max-Age=0; SameSite=Lax';

      // Redireciona para login substituindo histórico
      window.location.replace('/login');
    } catch (err) {
      console.error('Logout error:', err);
      window.location.replace('/login');
    }
  };

  return (
    <button
      onClick={handleLogout}
      disabled={loggingOut}
      className="text-sm text-slate-400 hover:text-red-400 disabled:opacity-50"
    >
      {loggingOut ? 'Logging out...' : 'Logout'}
    </button>
  );
}

