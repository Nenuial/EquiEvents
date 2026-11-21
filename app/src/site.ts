// equievents.ch's own pages, in the office's language (crates/server/src/site.rs, downloads.rs),
// and the language the app starts in.

import type { Language } from './i18n';

/** In the desktop app (as `api.inTauri`, kept here: api.ts uses this module). */
const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const LANGUAGES: Language[] = ['fr', 'de', 'it'];

/** The site's sign-in, sign-out and download pages. */
export const signInPage = (l: Language) => ({ fr: '/connexion', de: '/anmelden', it: '/accedi' })[l];
export const signOutPage = (l: Language) => ({ fr: '/deconnexion', de: '/abmelden', it: '/esci' })[l];
export const downloadPage = (l: Language) => ({ fr: '/telecharger', de: '/herunterladen', it: '/scarica' })[l];

const known = (code: string | null | undefined): Language | null => {
  const l = (code ?? '').trim().toLowerCase().slice(0, 2) as Language;
  return LANGUAGES.includes(l) ? l : null;
};

/** The office's language as chosen in the app, if it was. */
export function chosenLanguage(): Language | null {
  try {
    return known(localStorage.getItem('language'));
  } catch {
    return null;
  }
}

/** The language the app starts in (the user, 2026-11-21): online, the one last chosen, on the
 *  site or in the app (both keep `equievents_lang`); else the one chosen in the app; else the
 *  first of the three the browser or the system asks for; else German. */
export function firstLanguage(): Language {
  const site = inTauri ? null : known(document.cookie.match(/(?:^|;\s*)equievents_lang=([a-z]+)/)?.[1]);
  const asked = (typeof navigator !== 'undefined' ? (navigator.languages ?? [navigator.language]) : []).map(known).find(Boolean) ?? null;
  return site ?? chosenLanguage() ?? asked ?? 'de';
}

/** Online, the site keeps the office's language too: signed out, its pages come in it. */
export function rememberOnSite(l: Language) {
  if (inTauri) return;
  document.cookie = `equievents_lang=${l}; Path=/; SameSite=Lax; Max-Age=${365 * 24 * 3600}`;
}
