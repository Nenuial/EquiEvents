// Developer mode (Rosson's): an administrator's of equievents.ch only (the user, 2026-11-22),
// also while acting as another account in the online version; nobody else sees its switch,
// whatever this computer kept. When on: the scoring tables' definitions in a show, and the
// English of a refusal beside its translation. The core refuses its commands to anyone else.

import { createContext, useContext, useState } from 'react';

/** Whether the person at the app is an administrator (App.tsx provides it). */
export const AdministratorContext = createContext(false);

/** The same, for code outside React (the refusals). */
let administrator = false;
export function setAdministrator(on: boolean) {
  administrator = on;
}

function stored(): boolean {
  try {
    return localStorage.getItem('developer') === '1';
  } catch {
    return false;
  }
}

/** Developer mode on: an administrator who turned it on (this computer). */
export function developerOn(): boolean {
  return administrator && stored();
}

/** In React: whether the switch shows, and whether the mode is on. */
export function useDeveloper(): { allowed: boolean; on: boolean; turn: (on: boolean) => void } {
  const allowed = useContext(AdministratorContext);
  const [kept, setKept] = useState(stored);
  const turn = (on: boolean) => {
    setKept(on);
    try {
      localStorage.setItem('developer', on ? '1' : '0');
    } catch {
      /* a preference only */
    }
  };
  return { allowed, on: allowed && kept, turn };
}
