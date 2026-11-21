// Notes at the bottom of the screen: what was done (with undo), what went wrong, what is under
// way (a print being made). One at a time; the latest replaces the one before.

import { createContext, useCallback, useContext, useEffect, useState, type ComponentType, type ReactNode } from 'react';
import { CircleAlert, CircleCheck, Info, LoaderCircle, X } from 'lucide-react';
import { useT } from '../i18n';

export interface Toast {
  text: string;
  tone?: 'done' | 'error' | 'busy' | 'info';
  /** Instead of the tone's icon (an undo shows the undo arrow). */
  icon?: ComponentType<{ className?: string }>;
  action?: { label: string; onClick: () => void; title?: string };
}

const ToastContext = createContext<{ show: (t: Toast) => void; clear: () => void }>({ show: () => {}, clear: () => {} });

export const useToast = () => useContext(ToastContext);

export function ToastProvider({ children }: { children: ReactNode }) {
  const t = useT();
  const [toast, setToast] = useState<(Toast & { id: number }) | null>(null);
  const show = useCallback((t: Toast) => setToast({ ...t, id: Date.now() + Math.random() }), []);
  const clear = useCallback(() => setToast(null), []);
  useEffect(() => {
    if (!toast || toast.tone === 'error' || toast.tone === 'busy') return;
    const timer = setTimeout(() => setToast(null), 6000);
    return () => clearTimeout(timer);
  }, [toast]);
  const Icon = toast?.icon ?? (toast?.tone === 'error' ? CircleAlert : toast?.tone === 'busy' ? LoaderCircle : toast?.tone === 'info' ? Info : CircleCheck);
  return (
    <ToastContext.Provider value={{ show, clear }}>
      {children}
      {toast && (
        <div
          key={toast.id}
          role={toast.tone === 'error' ? 'alert' : 'status'}
          className={`fixed right-4 bottom-[max(1rem,env(safe-area-inset-bottom))] left-4 z-[60] flex animate-toast-in items-center gap-3 rounded-xl border px-4 py-3 text-sm shadow-pop sm:left-auto sm:max-w-md ${
            toast.tone === 'error' ? 'border-danger/30 bg-danger text-white' : 'border-white/10 bg-[#0b2234] text-white'
          }`}
        >
          <Icon
            className={`size-5 shrink-0 ${toast.tone === 'busy' ? 'animate-spin opacity-80' : toast.tone === 'error' ? '' : toast.tone === 'info' ? 'text-white/70' : 'text-[#5ee0a0]'}`}
            aria-hidden
          />
          <span className="min-w-0 flex-1 leading-snug">{toast.text}</span>
          {toast.action && (
            <button
              onClick={() => {
                toast.action?.onClick();
              }}
              title={toast.action.title}
              className="shrink-0 rounded-md bg-white/12 px-2.5 py-1 text-xs font-semibold hover:bg-white/20"
            >
              {toast.action.label}
            </button>
          )}
          <button onClick={clear} aria-label={t('close')} className="shrink-0 rounded-md p-1 text-white/60 hover:bg-white/10 hover:text-white">
            <X className="size-4" />
          </button>
        </div>
      )}
    </ToastContext.Provider>
  );
}
