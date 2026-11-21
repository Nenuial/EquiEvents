import { useEffect, useState } from 'react';
import { LanguageContext, type Language } from './i18n';
import Library from './screens/Library';
import ShowScreen from './screens/ShowScreen';
import Welcome, { accountHere } from './screens/Welcome';
import Accounts from './screens/Accounts';
import { ActingBand, staffMe } from './parts/staff';
import { ErrorNote, useLoad } from './parts/common';
import { useZoomKeys } from './parts/zoom';
import { ToastProvider } from './ui';

/** Where we are, kept in the address (#/show/<id>/<view>/<class>?peek=<class>&entry=<entry>) so a
 * reload stays put. `combined`: a ranking over several classes, its id in `class`; `entry`: an
 * entry marked on the class's start list (come from a rider's dialog). */
export interface Place {
  show?: string;
  /** The library's archives (no show). */
  archives?: boolean;
  /** « Comptes »: the accounts, for staff (the online version only). */
  accounts?: boolean;
  view: 'classes' | 'participants' | 'officials' | 'prints' | 'combined' | 'sending' | 'history';
  class?: string;
  peek?: string;
  entry?: number;
}

function parse(hash: string): Place {
  const [path, query] = hash.replace(/^#\/?/, '').split('?');
  const parts = path.split('/').filter(Boolean).map(decodeURIComponent);
  const params = new URLSearchParams(query ?? '');
  const peek = params.get('peek') ?? undefined;
  const entry = Number(params.get('entry')) || undefined;
  if (parts[0] === 'archives') return { view: 'classes', archives: true };
  if (parts[0] === 'accounts') return { view: 'classes', accounts: true };
  if (parts[0] !== 'show' || !parts[1]) return { view: 'classes' };
  const views: Place['view'][] = ['participants', 'officials', 'prints', 'combined', 'sending', 'history'];
  const view = views.find(v => v === parts[2]) ?? 'classes';
  return { show: parts[1], view, class: parts[3], peek, entry };
}

export function href(place: Place): string {
  if (!place.show) return place.archives ? '#/archives' : place.accounts ? '#/accounts' : '#/';
  let h = `#/show/${encodeURIComponent(place.show)}/${place.view}`;
  if (place.class) h += `/${encodeURIComponent(place.class)}`;
  const params = new URLSearchParams();
  if (place.peek) params.set('peek', place.peek);
  if (place.entry) params.set('entry', String(place.entry));
  if (params.size) h += `?${params}`;
  return h;
}

export default function App() {
  const [place, setPlace] = useState<Place>(() => parse(window.location.hash));
  const [language, setLanguage] = useState<Language>(() => (localStorage.getItem('language') as Language) || 'fr');
  // Nothing without an equievents.ch account (the user, 2026-11-21).
  const [accountRound, setAccountRound] = useState(0);
  const account = useLoad(accountHere, [accountRound]);
  const accountChanged = () => setAccountRound(r => r + 1);
  // The online version: staff, or acting as an account (the band on top of every screen).
  const hosted = !!account.data?.signed_in && !!account.data.hosted;
  const me = useLoad(() => (hosted ? staffMe() : Promise.resolve(null)), [hosted, accountRound]);
  const isStaff = !!me.data?.role;
  useZoomKeys();

  useEffect(() => {
    const onHash = () => setPlace(parse(window.location.hash));
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  }, []);

  const go = (next: Place) => {
    window.location.hash = href(next);
  };
  const switchLanguage = (l: Language) => {
    localStorage.setItem('language', l);
    setLanguage(l);
  };

  return (
    <LanguageContext.Provider value={language}>
      {/* Inside the language: a toast's own words (close) are the office's. */}
      <ToastProvider>
        <div className="flex h-full flex-col">
          {me.data && <ActingBand me={me.data} />}
          <div className="min-h-0 flex-1">
            {!account.data ? (
              <ErrorNote error={account.error} />
            ) : !account.data.signed_in ? (
              <Welcome server={account.data.server} done={accountChanged} language={language} setLanguage={switchLanguage} />
            ) : place.show ? (
              <ShowScreen place={place as Place & { show: string }} go={go} language={language} setLanguage={switchLanguage} />
            ) : place.accounts && me.data && isStaff ? (
              <Accounts me={me.data} language={language} />
            ) : (
              <Library open={show => go({ show, view: 'classes' })} archives={!!place.archives} language={language} setLanguage={switchLanguage} accountChanged={accountChanged} staff={isStaff} />
            )}
          </div>
        </div>
      </ToastProvider>
    </LanguageContext.Provider>
  );
}
