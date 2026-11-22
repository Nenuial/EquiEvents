// Before anything: an equievents.ch account (the user, 2026-11-21: the apps do not work without
// one). Signing in or making one, here or on the site; once signed in, this computer keeps the
// account, also offline, until it signs out.

import { call } from '../api';
import { useT, type Language } from '../i18n';
import { AccountForms } from '../parts/online';
import { LanguagePicker, ThemeButton } from '../shell/ShowShell';
import { Card, Logo } from '../ui';

export default function Welcome({ server, done, language, setLanguage }: { server: string; done: () => void; language: Language; setLanguage: (l: Language) => void }) {
  const t = useT();
  return (
    <div className="pane bg-page">
      <div className="relative overflow-hidden bg-nav pb-24 text-white">
        <div
          className="pointer-events-none absolute inset-0 bg-[radial-gradient(80%_120%_at_100%_0%,rgb(242_90_93/0.22),transparent_55%),radial-gradient(60%_90%_at_0%_100%,rgb(31_111_196/0.18),transparent_60%)]"
          aria-hidden
        />
        <div className="relative mx-auto flex max-w-6xl items-center gap-2 px-4 pt-[max(1rem,env(safe-area-inset-top))] pb-2 sm:px-6">
          <Logo dark />
          <div className="flex-1" />
          <ThemeButton className="text-white/70 hover:bg-white/10 hover:text-white" />
          <LanguagePicker language={language} setLanguage={setLanguage} />
        </div>
        <div className="relative mx-auto max-w-md px-4 pt-10 sm:pt-14">
          <h1 className="text-[28px] leading-tight font-bold tracking-tight sm:text-[32px]">{t('welcome_title')}</h1>
          <p className="mt-2 text-sm text-white/70">{t('welcome_hint')}</p>
        </div>
      </div>
      <main className="relative mx-auto -mt-16 max-w-md px-4 pb-10">
        <Card className="p-5 sm:p-6">
          <AccountForms done={done} />
        </Card>
        <p className="mt-3 text-center text-xs text-ink-4">{server}</p>
      </main>
    </div>
  );
}

/** Whether this computer has an account signed in (no network: the account it keeps). */
export const accountHere = () => call<{ server: string; signed_in: boolean; hosted?: boolean }>('account');
