// Staff (the user, 2026-11-21), in the online version only: who the session is (an
// administrator, support or neither) and whom it acts as. While acting as an account, a band on
// top of every screen says so and brings back to one's own; its changes are signed with both
// names (crates/server/src/admin.rs).

import { useState } from 'react';
import { UserRoundCog } from 'lucide-react';
import { staff } from '../api';
import { useT, type Key } from '../i18n';
import { Button } from '../ui';

export type StaffRole = 'admin' | 'support' | '';

export interface StaffMe {
  id: number;
  name: string;
  role: StaffRole;
  acting_as: { id: number; name: string; email: string } | null;
}

export interface StaffAccount {
  id: number;
  name: string;
  email: string;
  role: StaffRole;
  created_at: string;
  last_seen: string | null;
  owns: number;
  works_on: number;
}

export interface StaffDetails {
  account: StaffAccount;
  owns: { id: string; name: string; collaborators: number }[];
  works_on: { id: string; name: string; owner: string }[];
}

/** A show on equievents.ch as the panel lists it. */
export interface StaffShow {
  id: string;
  name: string;
  event_id: number | null;
  last_day: string | null;
  archived: boolean;
  owner: { id: number; name: string; email: string } | null;
  collaborators: number;
}

export interface StaffShowDetails {
  show: StaffShow;
  people: { id: number; name: string; email: string; role: 'owner' | 'collaborator' }[];
}

/** The session: in the online version only (elsewhere, none). */
export const staffMe = () => staff<StaffMe>('me');

/** The whole app again, as the account the session now works for (its office, its shows). */
export function reloadAs() {
  window.location.hash = '#/';
  window.location.reload();
}

/** A refusal of the panel in the office's language. */
export function useStaffRefusal() {
  const t = useT();
  return (e: unknown) => {
    const code = (e as { code?: string })?.code ?? 'error';
    return t(`staff_refused_${code}` as Key) ?? t('staff_refused_error');
  };
}

/** On top of every screen while acting as an account: whose, how changes are signed, the way back. */
export function ActingBand({ me }: { me: StaffMe }) {
  const t = useT();
  const refusal = useStaffRefusal();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!me.acting_as) return null;
  const back = async () => {
    setBusy(true);
    setError(null);
    try {
      await staff('stop', 'POST');
      reloadAs();
    } catch (e) {
      setError(refusal(e));
      setBusy(false);
    }
  };
  return (
    <div role="status" className="shrink-0 border-b border-warn/40 bg-warn/15 pt-[max(0.5rem,env(safe-area-inset-top))] pb-2 text-ink">
      <div className="mx-auto flex max-w-6xl flex-wrap items-center gap-x-3 gap-y-2 px-4 sm:px-6">
        <UserRoundCog className="size-[18px] shrink-0 text-warn" aria-hidden />
        <div className="min-w-0 flex-1 basis-56 text-[13px]">
          <span>{t('staff_acting')} </span>
          <span className="font-semibold">{me.acting_as.name}</span>
          <span className="text-ink-3"> · {me.acting_as.email}</span>
          <div className="text-xs text-ink-2">{t('staff_acting_hint').replace('{name}', me.acting_as.name).replace('{me}', me.name)}</div>
          {error && <div className="text-xs font-semibold text-danger">{error}</div>}
        </div>
        <Button size="sm" onClick={back} loading={busy}>
          {t('staff_stop')}
        </Button>
      </div>
    </div>
  );
}
