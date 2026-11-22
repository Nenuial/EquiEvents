// The core's refusals in the office's language (the user, 2026-11-15: the interface entirely in
// French, German and Italian). The core answers in English; each of its messages, with `{}` for
// the values it puts in, has its French, German and Italian in refusals.json ({1}, {2}… for the
// values), which a test of the core keeps complete (crates/office/tests/refusals.rs). A message
// made of messages ("class 12: movement 3: …", or several joined by "; ") is translated part by
// part. One the catalogue does not know says that the action could not be done; its English stays
// in the error log (Réglages › Avancé) and shows in developer mode (an administrator's).

import { developerOn } from './developer';
import catalogue from './refusals.json';
import { firstLanguage } from './site';

type Language = 'fr' | 'de' | 'it';
type Texts = Record<Language, string>;

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

// The most specific first: more words of their own, then fewer values.
const patterns = Object.entries(catalogue as Record<string, Texts>)
  .map(([english, texts]) => {
    const parts = english.split('{}');
    return { re: new RegExp(`^${parts.map(escape).join('(.+?)')}$`, 's'), texts, holes: parts.length - 1, own: parts.join('').length };
  })
  .sort((a, b) => b.own - a.own || a.holes - b.holes);

/** The office's language (App.tsx keeps it; before it was chosen, the one the app starts in). */
function language(): Language {
  return firstLanguage();
}

/** A message in `lang`, or `null` when the catalogue does not know it. */
export function translate(message: string, lang: Language = language(), depth = 0): string | null {
  const text = message.trim();
  // Several refusals at once (changes the show refused, joined by the core), when each is one;
  // else the whole ("class 12 is final; reopen it first").
  const parts = text.split('; ');
  if (parts.length > 1) {
    const each = parts.map(p => translate(p, lang, depth + 1));
    if (each.every(e => e !== null)) return each.map((e, i) => (i < each.length - 1 ? e!.replace(/\.$/, '') : e)).join(' · ');
  }
  for (const p of patterns) {
    const m = p.re.exec(text);
    if (!m) continue;
    let out = p.texts[lang] || p.texts.fr;
    for (let i = 1; i <= p.holes; i++) {
      const value = m[i];
      // A value that is a message itself (a class's refusal inside "class 12: …"): translated,
      // without its full stop; names and numbers as they are.
      const inner = depth < 3 && /\p{L}{3}/u.test(value) ? translate(value, lang, depth + 1) : null;
      out = out.split(`{${i}}`).join(inner ? inner.replace(/\.$/, '') : value);
    }
    return out;
  }
  return null;
}

/** What the office reads for a refusal of the core: its translation, else that it could not be
 *  done (the English after it in developer mode). */
export function refusal(message: string): string {
  const lang = language();
  const known = translate(message, lang);
  if (known !== null) return known;
  const fallback = (catalogue as Record<string, Texts>)['unknown error']?.[lang] ?? 'Impossible.';
  return developerOn() ? `${fallback} (${message})` : fallback;
}
