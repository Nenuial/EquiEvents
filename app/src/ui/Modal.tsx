// Dialogs: what is not the screen's main work opens in front of it, never in a panel beside or
// under it (P. Burkhard, 2026-11-14). On a phone a dialog comes up from the bottom and takes
// the screen's width. Esc or a click outside closes it; focus stays inside while it is open.

import { useRef, useState, type ComponentType, type ReactNode } from 'react';
import { Dialog } from 'radix-ui';
import { X } from 'lucide-react';
import { useT } from '../i18n';
import { Button, IconButton } from './Button';

export type ModalSize = 'sm' | 'md' | 'lg' | 'xl' | 'full';

const widths: Record<ModalSize, string> = {
  sm: 'sm:w-[min(92vw,26rem)]',
  md: 'sm:w-[min(92vw,36rem)]',
  lg: 'sm:w-[min(94vw,48rem)]',
  xl: 'sm:w-[min(95vw,68rem)]',
  full: 'sm:w-[96vw] sm:h-[92vh]',
};

type Icon = ComponentType<{ className?: string }>;

/** Whether a dialog or a menu is open: global key shortcuts leave it its keys (Esc, letters). */
export function overlayOpen(): boolean {
  return document.querySelector('[role="dialog"][data-state="open"], [role="menu"]') !== null;
}

export function Modal({
  open,
  onClose,
  title,
  description,
  icon: Icon,
  tone = 'plain',
  size = 'md',
  children,
  footer,
  bodyClass = '',
  focusFirst = true,
}: {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  description?: ReactNode;
  icon?: Icon;
  tone?: 'plain' | 'danger' | 'accent' | 'warn';
  size?: ModalSize;
  children?: ReactNode;
  /** Buttons at the bottom (the main action last, on the right). */
  footer?: ReactNode;
  bodyClass?: string;
  /** Put the cursor in the first field (else the dialog itself takes the focus). */
  focusFirst?: boolean;
}) {
  const t = useT();
  const content = useRef<HTMLDivElement>(null);
  // A menu's "…" says a dialog follows; the dialog's own title does not need it.
  const heading = typeof title === 'string' ? title.replace(/(…|\.\.\.)$/, '') : title;
  const tile = tone === 'danger' ? 'bg-danger/12 text-danger' : tone === 'accent' ? 'bg-accent-soft text-accent' : tone === 'warn' ? 'bg-warn/14 text-warn' : 'bg-sunken text-ink-2';
  return (
    <Dialog.Root open={open} onOpenChange={o => !o && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 animate-fade-in bg-[rgb(2_10_18/0.55)] backdrop-blur-[3px]" />
        <Dialog.Content
          onOpenAutoFocus={e => {
            // The first field, else the dialog itself: never the close button (its tooltip).
            e.preventDefault();
            const box = content.current;
            const first = focusFirst ? box?.querySelector<HTMLElement>('[data-autofocus], input:not([type=hidden]):not([disabled]), select:not([disabled]), textarea:not([disabled])') : null;
            (first ?? box)?.focus({ preventScroll: true });
          }}
          ref={content}
          tabIndex={-1}
          className={`fixed inset-x-0 bottom-0 z-50 flex max-h-[94dvh] animate-sheet-in flex-col rounded-t-2xl border border-line bg-card shadow-pop outline-none sm:inset-x-auto sm:bottom-auto sm:left-1/2 sm:top-[7vh] sm:max-h-[86vh] sm:-translate-x-1/2 sm:animate-modal-in sm:rounded-2xl ${widths[size]} ${size === 'full' ? 'sm:top-[4vh] sm:max-h-[92vh]' : ''}`}
        >
          <div className="mx-auto mt-2 h-1 w-10 shrink-0 rounded-full bg-line-strong sm:hidden" aria-hidden />
          <div className="flex shrink-0 items-start gap-3 px-5 pt-4 pb-3 sm:px-6 sm:pt-5">
            {Icon && (
              <div className={`grid size-10 shrink-0 place-items-center rounded-xl ${tile}`}>
                <Icon className="size-5" />
              </div>
            )}
            <div className="min-w-0 flex-1 pt-0.5">
              <Dialog.Title className="text-[17px] leading-snug font-semibold tracking-tight text-ink">{heading}</Dialog.Title>
              {description ? (
                <Dialog.Description className="mt-0.5 text-[13px] text-ink-3">{description}</Dialog.Description>
              ) : (
                <Dialog.Description className="sr-only">{typeof heading === 'string' ? heading : ''}</Dialog.Description>
              )}
            </div>
            <Dialog.Close asChild>
              <IconButton icon={X} label={t('close')} kbd="Esc" size="sm" className="-mt-0.5 -mr-1.5" />
            </Dialog.Close>
          </div>
          <div className={`min-h-0 flex-1 overflow-auto px-5 pb-5 sm:px-6 ${bodyClass}`}>{children}</div>
          {footer && (
            <div className="flex shrink-0 flex-wrap items-center justify-end gap-2 rounded-b-2xl border-t border-line bg-raised px-5 py-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] sm:px-6">
              {footer}
            </div>
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

/** Asking before something that takes much away (undo still brings it back), or that needs a
 *  reason (`reason`: the field's placeholder; the confirmation waits for it). */
export function ConfirmModal({
  open,
  onClose,
  title,
  message,
  confirm,
  onConfirm,
  danger = false,
  icon,
  reason,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  message?: ReactNode;
  confirm: string;
  onConfirm: (reason: string) => void | Promise<unknown>;
  danger?: boolean;
  icon?: Icon;
  reason?: string;
}) {
  const t = useT();
  const [text, setText] = useState('');
  const [busy, setBusy] = useState(false);
  const go = async () => {
    if (reason !== undefined && !text.trim()) return;
    setBusy(true);
    try {
      await onConfirm(text.trim());
    } finally {
      setBusy(false);
      setText('');
    }
  };
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      icon={icon}
      tone={danger ? 'danger' : 'accent'}
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button variant={danger ? 'danger-solid' : 'primary'} loading={busy} disabled={reason !== undefined && !text.trim()} onClick={go}>
            {confirm}
          </Button>
        </>
      }
    >
      {message && <div className="text-sm text-ink-2">{message}</div>}
      {reason !== undefined && <input data-autofocus value={text} onChange={e => setText(e.target.value)} onKeyDown={e => e.key === 'Enter' && go()} placeholder={reason} className="field mt-3" />}
    </Modal>
  );
}
