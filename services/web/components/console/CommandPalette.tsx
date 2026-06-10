'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useRouter } from 'next/navigation';
import { cn } from '@/lib/utils';

interface Command {
  id: string;
  label: string;
  hint?: string;
  run: () => void;
}

/**
 * Global ⌘K / Ctrl-K command palette — navigate the console from the keyboard.
 * Dependency-free: a fixed overlay with a filtered list, arrow-key selection,
 * Enter to run. Mounted once in the console shell. Symbol jumps and control
 * actions can join the command list as they're wired.
 */
export function CommandPalette() {
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const commands = useMemo<Command[]>(
    () => [
      { id: 'console', label: 'Go to Console', hint: 'g c', run: () => router.push('/console') },
      { id: 'strategy', label: 'Go to Strategy', hint: 'g s', run: () => router.push('/strategy') },
      { id: 'trades', label: 'Go to Trades', hint: 'g t', run: () => router.push('/trades') },
      { id: 'analysis', label: 'Go to Analysis', hint: 'g a', run: () => router.push('/analysis') },
      { id: 'system', label: 'Go to System · controls', hint: 'g y', run: () => router.push('/system') },
    ],
    [router]
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter(c => c.label.toLowerCase().includes(q));
  }, [commands, query]);

  const close = useCallback(() => {
    setOpen(false);
    setQuery('');
    setActive(0);
  }, []);

  // Global hotkey: ⌘K / Ctrl-K toggles; Esc closes.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setOpen(o => !o);
      } else if (e.key === 'Escape') {
        setOpen(false);
      }
    };
    const onOpen = () => setOpen(true);
    window.addEventListener('keydown', onKey);
    window.addEventListener('command-palette:open', onOpen);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('command-palette:open', onOpen);
    };
  }, []);

  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  if (!open) return null;

  function runAt(i: number) {
    const cmd = filtered[i];
    if (!cmd) return;
    close();
    cmd.run();
  }

  return (
    <div
      className="fixed inset-0 z-[100] flex items-start justify-center bg-black/50 pt-[15vh]"
      onClick={close}
    >
      <div
        className="w-full max-w-lg overflow-hidden rounded-xl border border-border bg-card shadow-2xl"
        onClick={e => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          value={query}
          onChange={e => {
            setQuery(e.target.value);
            setActive(0);
          }}
          onKeyDown={e => {
            if (e.key === 'ArrowDown') {
              e.preventDefault();
              setActive(a => Math.min(filtered.length - 1, a + 1));
            } else if (e.key === 'ArrowUp') {
              e.preventDefault();
              setActive(a => Math.max(0, a - 1));
            } else if (e.key === 'Enter') {
              e.preventDefault();
              runAt(active);
            }
          }}
          placeholder="Jump to…"
          className="w-full border-b border-border bg-transparent px-4 py-3 text-sm text-foreground outline-none placeholder:text-muted-foreground"
        />
        <ul className="max-h-72 overflow-y-auto py-1">
          {filtered.length === 0 ? (
            <li className="px-4 py-6 text-center text-sm text-muted-foreground">
              No matches
            </li>
          ) : (
            filtered.map((c, i) => (
              <li key={c.id}>
                <button
                  type="button"
                  onMouseEnter={() => setActive(i)}
                  onClick={() => runAt(i)}
                  className={cn(
                    'flex w-full items-center justify-between px-4 py-2.5 text-left text-sm transition-colors',
                    i === active
                      ? 'bg-primary/10 text-foreground'
                      : 'text-muted-foreground'
                  )}
                >
                  <span>{c.label}</span>
                  {c.hint && (
                    <span className="font-mono text-[11px] text-muted-foreground">
                      {c.hint}
                    </span>
                  )}
                </button>
              </li>
            ))
          )}
        </ul>
      </div>
    </div>
  );
}
