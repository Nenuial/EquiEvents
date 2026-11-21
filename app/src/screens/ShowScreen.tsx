// An open show: its frame (ShowShell) and the part shown. Changes go through ChangesProvider.

import { useMemo, useState } from 'react';
import { call, type ShowView } from '../api';
import type { Place } from '../App';
import { useT, type Language } from '../i18n';
import { ChangesProvider, useChanges } from '../parts/changes';
import { ErrorNote, useLoad } from '../parts/common';
import { SyncBadge } from '../parts/online';
import { ReferenceButton, SettingsModal } from '../parts/settings';
import { ShowShell } from '../shell/ShowShell';
import { Loading } from '../ui';
import { ClassPage } from './ClassPage';
import { Classes } from './Classes';
import { CombinedScreen } from './Combined';
import History from './History';
import Officials from './Officials';
import { Participants } from './Participants';
import { Prints } from './Prints';
import Sending from './Sending';

type Go = (place: Place) => void;

export default function ShowScreen(props: { place: Place & { show: string }; go: Go; language: Language; setLanguage: (l: Language) => void }) {
  return (
    <ChangesProvider show={props.place.show}>
      <Show {...props} />
    </ChangesProvider>
  );
}

function Show({ place, go, language, setLanguage }: { place: Place & { show: string }; go: Go; language: Language; setLanguage: (l: Language) => void }) {
  const t = useT();
  const { version } = useChanges();
  const show = useLoad(() => call<ShowView>('show', { show: place.show }), [place.show, version]);
  const [settings, setSettings] = useState(false);
  // All classes of the show, to find a class's sections.
  const classes = useMemo(() => show.data?.days.flatMap(d => d.classes) ?? [], [show.data]);
  const data = show.data;
  return (
    <ShowShell
      show={place.show}
      event={data?.event}
      place={place}
      go={go}
      language={language}
      setLanguage={setLanguage}
      sync={look => <SyncBadge show={place.show} openShow={id => go({ show: id, view: 'classes' })} look={look} />}
      settings={() => setSettings(true)}
      extra={icon => <ReferenceButton show={place.show} icon={icon} />}
    >
      {show.error && <ErrorNote error={show.error} />}
      {!data && !show.error && (
        <div className="h-full overflow-hidden" title={t('loading')}>
          <Loading />
        </div>
      )}
      {data && place.view === 'classes' && !place.class && <Classes show={data} go={go} />}
      {data && place.view === 'classes' && place.class && <ClassPage key={place.class} show={place.show} classId={place.class} entry={place.entry} classes={classes} go={go} language={language} />}
      {data && place.view === 'participants' && <Participants show={place.show} peek={place.peek} classes={classes} go={go} />}
      {data && place.view === 'officials' && <Officials show={place.show} language={language} />}
      {data && place.view === 'prints' && <Prints show={data} />}
      {data && place.view === 'sending' && <Sending show={place.show} />}
      {data && place.view === 'history' && <History show={place.show} />}
      {data && place.view === 'combined' && place.class && <CombinedScreen show={data} id={place.class} go={go} />}
      <SettingsModal open={settings} onClose={() => setSettings(false)} language={language} setLanguage={setLanguage} />
    </ShowShell>
  );
}
