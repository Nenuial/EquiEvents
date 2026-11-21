// The light and the dark theme: the office's choice on this computer (`theme`: system, light,
// dark), written on <html> as `data-theme` (index.css), the system's followed while it is "system".
// The accent too (`accent`: coral by default, or pétrole, bordeaux, laiton, cobalt), as
// `data-accent`.

import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';

export type ThemeChoice = 'system' | 'light' | 'dark';
export type Accent = 'corail' | 'petrole' | 'bordeaux' | 'laiton' | 'cobalt';
/** The accents, coral first (the default), each with its colour in the light theme (a swatch). */
export const ACCENTS: { value: Accent; color: string }[] = [
  { value: 'corail', color: '#e5484d' },
  { value: 'petrole', color: '#0b7179' },
  { value: 'bordeaux', color: '#9b2743' },
  { value: 'laiton', color: '#8c6214' },
  { value: 'cobalt', color: '#2d52cc' },
];

function storedAccent(): Accent {
  try {
    const v = localStorage.getItem('accent');
    return ACCENTS.find(a => a.value === v)?.value ?? 'corail';
  } catch {
    return 'corail';
  }
}

function stored(): ThemeChoice {
  try {
    const v = localStorage.getItem('theme');
    return v === 'light' || v === 'dark' ? v : 'system';
  } catch {
    return 'system';
  }
}

const dark = () => window.matchMedia('(prefers-color-scheme: dark)').matches;

const ThemeContext = createContext<{ choice: ThemeChoice; resolved: 'light' | 'dark'; choose: (c: ThemeChoice) => void; accent: Accent; chooseAccent: (a: Accent) => void }>({
  choice: 'system',
  resolved: 'light',
  choose: () => {},
  accent: 'corail',
  chooseAccent: () => {},
});

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [choice, setChoice] = useState<ThemeChoice>(stored);
  const [accent, setAccent] = useState<Accent>(storedAccent);
  const [system, setSystem] = useState<'light' | 'dark'>(() => (dark() ? 'dark' : 'light'));
  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const change = () => setSystem(media.matches ? 'dark' : 'light');
    media.addEventListener('change', change);
    return () => media.removeEventListener('change', change);
  }, []);
  const resolved = choice === 'system' ? system : choice;
  useEffect(() => {
    document.documentElement.dataset.theme = resolved;
  }, [resolved]);
  useEffect(() => {
    document.documentElement.dataset.accent = accent;
  }, [accent]);
  const chooseAccent = (a: Accent) => {
    setAccent(a);
    try {
      localStorage.setItem('accent', a);
    } catch {
      /* a preference only */
    }
  };
  const choose = (c: ThemeChoice) => {
    setChoice(c);
    try {
      localStorage.setItem('theme', c);
    } catch {
      /* a preference only */
    }
  };
  return <ThemeContext.Provider value={{ choice, resolved, choose, accent, chooseAccent }}>{children}</ThemeContext.Provider>;
}

export const useTheme = () => useContext(ThemeContext);
