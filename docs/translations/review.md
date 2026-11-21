# Translation review: Rosson (French, Italian) and EquiEvents (French)

Reviewed 14 November 2026. Read-only review; nothing in the code was changed.

**Sources**

- Rosson 3.3.15: `vendor/rosson-3.3.15/src/renderer/app/i18n/{de,fr,it}.js` (every key compared across the three languages) and the print services in `services/print/` (plus `layout.print.factory.js`).
- EquiEvents: `app/src/i18n.tsx` (`fr` against `de`), `crates/office/src/prints.rs`, `crates/office/src/history.rs`, `crates/server/src/site.rs`, `crates/server/src/notices.rs`.

**Reference for "official" wording.** Swiss Equestrian's own three-language master data is in the golden corpus (`golden/corpus/*/snapshot.json`). It contains role names, scoring tables, retirement reasons, class categories and dressage test texts, all from Swiss Equestrian. Where this report says "(SE)", the term comes from there. That data has its own slips (Italian `manches`, `winning round`, some French left in German), so the report follows it only where it is sound.

**How to read the tables.** The line numbers point to `fr.js` (or `it.js`) unless another file is named. The German column is Rosson's original, from `de.js`. A key used in a print is marked "print" when that matters.

---

## 1. Summary

**Rosson French is usable but plainly machine-made in places.** Of 1,021 German keys, 1,014 have a French value. **About 230 values are flagged**: 79 for wrong meaning or machine output (2.1), 22 terminology and consistency issues covering about 80 keys (2.2), 52 grammar, agreement and typo errors (2.3), 7 missing keys (2.4) and 8 hard-coded texts in the prints (2.5). Roughly one value in five has a problem. The worst ones are in what people see during a show: driving start lists and rankings, control sheets, dressage judge sheets, print footers and confirmation dialogs.

The main patterns:

1. **One verb for three actions.** *Hochladen* (upload to Swiss Equestrian), *herunterladen* (download) and *laden* (load locally) are all « télécharger ». A French user cannot tell whether results were sent or fetched (about 20 keys).
2. **German words with several meanings, translated with the wrong one**:
   - *Zeit* → « Heure » for a round time, which gives « Heure Os » in driving rankings.
   - *Gespann* → « Équipe »; *Paar* → « paire »; *Tempo* → « Cadence ».
   - *nach* → « après »; *Prüfung* → « examen »; *Teilprüfung* → « Essai partiel ».
   - *Richter/in (Position)* → « Juge en (Position) », where the German feminine ending was read as a preposition.
   - *Ver.-Nr* (the show number) → « Épr. n° », printed in every footer.
3. **Nonsense left in**: « Il dirige une migration », « Pas des results », « Défausser », « Os », « Par hasard », « en attendant », « Prix naturelle ».
4. **Wrong tense in delete messages**: the warning shown *before* deleting says « a été effacé », and the message shown *after* says « sera effacé ».
5. **Inconsistency**:
   - *Rangliste* is « liste de résultat » and « classement ».
   - *Strafpunkte* has four forms.
   - *Startnummer* is « numéro de start » / « no de start » / « départ n° » / « n° de départ ».
   - *Gewinnpunkte* is « Somme de points » / « SOP » / « SoP ».
   - *speichern*, *löschen* and *erstellen* each have two verbs.
6. **Agreement and typos**: about 50 values such as « résultats partielles », « est été téléchargé », « Nouveau Engagements », « Persone inconnu », « Déscription », « Pénaltiés ».
7. **Leftovers**:
   - A « FR :) » placeholder that users see in the history.
   - « Bodentest » left in German.
   - 7 keys missing (they show in German).
   - French text inside `de.js` (`OF.CONFIRM_DELETE_CHILDREN`).

**Rosson Italian barely exists.** Only **53 of 1,021 keys (5 %)** are translated. 19 of those are the developer error log and one is a bare `{{veranstaltung}}`. Everything else falls back to German (`fallbackLanguage('de')`, `de.js:1225`). Italian prints also get **French** dates and **German** weekday and month names. Of the 53 Italian values, 12 are flagged.

**EquiEvents French is natural and mostly correct.** It reads like French written by someone who knows the sport. **46 items are flagged**: 30 in `i18n.tsx`, 13 in `prints.rs` and 3 on the server side, a few of them the same string in two places. The most important ones:

- The dressage judge sheet labels in `prints.rs` were copied from Rosson, errors included (« Numéro de start », « Juge en (Position) », « Résultat en pourcent »).
- A few calques: « version du pilote », « Reprendre ses rangs », « Toute l'équipe ne part pas » (ambiguous).
- Some term choices to align with Swiss Equestrian: « manche » rather than « parcours » for *Umgang*, « origines » rather than « ascendance », one word for *Verein*.
- Spaces before « : ? ! » are plain spaces, never non-breaking ones.

### The five most serious problems

1. **Italian is missing in Rosson.** 95 % of screens and prints show German to an Italian-speaking office, with French dates.
2. **« Télécharger » for both upload and download** on the screens whose job is to send start lists and results to Swiss Equestrian (2.1 row 7).
3. **Wrong words on official paper**:
   - « Juge en (Position) » on every dressage judge sheet, also in EquiEvents `prints.rs:558`.
   - « Épr. n° » for the show number in every print footer.
   - « Équipe » for *Gespann* on driving start lists, rankings and control sheets.
   - « Essai partiel » for *Teilprüfung*; « Heure Os » in driving rankings.
4. **Driving control sheets (forms the judges fill in) say the wrong thing**:
   - Derby penalties in seconds are called « points ».
   - Penalty line 19 mixes two rules.
   - « Élimination coureur »; « 1ère montre ».
   - French says 1/100 s where German says 1/1000 s.
5. **Misleading dialogs and messages**:
   - Delete warnings in the wrong tense.
   - The deletion warning that results may be lost is dropped in French (`OF.CONFIRM_DELETE_CHILDREN`).
   - Import errors replaced by « Fichier illisible » or « C'est le fichier master ».
   - « Il dirige une migration », and the « FR :) » placeholder in the history.

---

## 2. Rosson French

### 2.1 Wrong meaning, machine output, nonsense

| # | Key (fr.js line) | German | Rosson French | Why it is wrong | Proposed French |
|---|---|---|---|---|---|
| 1 | DISCARD (11) | Verwerfen | Défausser | « Défausser » is discarding a card in a card game. It is also quoted in WARNING_RESULTATE_PRO_RICHTER (142). | Ignorer les modifications |
| 2 | INFO_MIGRATION (28) | Es läuft eine Datenmigration auf Version {{ version }}. Wir bitten um etwas Geduld… | Il dirige une migration de données vers la version {{ version }}… | « Es läuft » became « il dirige » (he manages). | Migration des données vers la version {{ version }} en cours. Merci de patienter et surtout de ne pas fermer Rosson. |
| 3 | HOCHGELADEN_AM (35) | aktualisiert am | conclu le | « Conclu » means concluded (a deal). | mis à jour le |
| 4 | NICHT_HOCHGELADEN (37) | noch nicht hochgeladen | pas conclu | Same error as row 3. | pas encore transmis |
| 5 | ABGESCHLOSSEN_AM (36) | Abgeschlossen am | Close le | « Close » is an adjective (*porte close*). | Clôturée le |
| 6 | KEINE_RESULTATE_FUER_UPLOAD (38) | Diese Zwischenprüfung hat noch keine Resultate. | Pas des results | English word, wrong article, meaning lost. | Cette épreuve n'a pas encore de résultats. |
| 7 | All upload / load keys: 39, 41, 42, 45, 89, 90, 166, 459, 460, 461, 462, 505, 533, 538, 539 (upload); 163, 473, 474, 581 (load) | hochladen / laden | télécharger, téléchargement, téléchargé(e)s | In French « télécharger » is understood as *download*. Rosson also uses it for real downloads (478–483), so sending to Swiss Equestrian and fetching from it read the same. | hochladen → « transmettre / envoyer (à Swiss Equestrian) », e.g. « Transmettre les résultats », « Les listes de départ ont été transmises. »; laden → « charger »; herunterladen → « télécharger » |
| 8 | ZU_WECHSELNDES_PFERD, ZU_WECHSELNDER_REITER (100, 101) | Zu wechselnde Pferde / Zu wechselnder Reiter | Changement de cheval / Changement de cavalier | These are field labels for the horse or rider *to be replaced*. The French repeats the dialog title. | Cheval à remplacer / Cavalier à remplacer |
| 9 | WECHSEL_AUF_NICHT_DEFINITIV (128) | Wechsel auf abgemeldeten, zurückgewiesenen oder falsch gemeldeten Nennungen nicht möglich! | Les changements sur les engagements annulés ou remboursés ne sont pas possible! | « Remboursés » is invented, and the statuses are wrong. | Impossible de modifier un engagement désinscrit, refusé ou erroné ! |
| 10 | WERTUNG_TEILPRUEFUNGEN (133) | Wertung Teilprüfungen (Punktegleichheit) | Barème épreuves partielles (égalité de points) | *Wertung* here means which partial test decides a tie, not a scoring table. | Prise en compte des épreuves partielles (en cas d'égalité de points) |
| 11 | WERTUNG_NACH (176) | Rangliste nach | Liste de résultat à | « nach » rendered as « à ». | Classement selon |
| 12 | FIRST_STARTING_TIME (184) | Erste Startzeit | 1ère temps du départ | A clock time, not a duration; wrong gender. | Heure du premier départ |
| 13 | ZUFALL (194) | Zufall (sort option) | Par hasard | « Par hasard » means *by accident*. | Tirage au sort |
| 14 | STRAFPUNKTE_ABWUERFE (209) | Strafpunkte Abwürfe (driving derby, field in seconds) | Points de pénalités décroche | « Décroche » means nothing here. | Pénalités pour éléments tombés (s) |
| 15 | GRUPPIERT_NACH_AP (218) | Gruppiert nach Ausschreibungsprüfung | Regroupés après épreuve | « nach » means *by*, not *after*. | Regroupés par épreuve de l'avant-programme |
| 16 | REIHENFOLGE_NACH_FOLGEPRUEFUNG (220) | Reihenfolge nach Folgeprüfung anpassen | Ordre de épreuve suivante | The verb is missing; ungrammatical. | Adapter l'ordre selon l'épreuve suivante |
| 17 | BONUS_ZEIT (260), GESPANN (718, 1097), GESP (720) — print: driving start lists, rankings, control sheets | Bonuszeit für Gespanne / Gespann / Gesp. | Temps de bonus pour Équipe / Équipe / Équipe | A *Gespann* is a driving turnout (« attelage »). « Équipe » collides with *Equipe* (team). | Temps bonus pour les attelages / Attelage / Att. |
| 18 | TEILPRUEFUNG (717) — print: driving start lists and rankings | Teilprüfung | Essai partiel | « Essai » means attempt. Rosson itself says « épreuve partielle » elsewhere. | Épreuve partielle |
| 19 | ZEIT (414) — jumping result field, driving ranking | Zeit | Heure | This is the time of the round, not the clock time. | Temps |
| 20 | HN (1014) — print: driving ranking, after ZEIT | Hn (Hindernisse) | Os | « Os » means bone. Together with row 19 it prints « Heure Os ». | obst. (so « Temps obst. ») |
| 21 | UHR (318) | Uhr (after "HH:mm") | Heures | Gives « 10:30 Heures ». | h (or drop the word) |
| 22 | LOGO_WILL_BE_REMOVED (361), VERANSTALTUNG_WILL_BE_DELETED (433), PROGRAMM_GELOESCHT (526) | Das Logo wird gelöscht / Die Veranstaltung wird lokal gelöscht / Das Programm wurde gelöscht | Le logo a été effacé / La manifestation a été localement effacée / Le programme sera effacé | Tenses swapped: the warning says it is already done, and the confirmation says it will be. | Le logo sera supprimé. / La manifestation sera supprimée de cet ordinateur. / Le programme a été supprimé. |
| 23 | ADRESSLISTE (443) | Adressliste | Adresse | One address instead of a list. | Liste d'adresses |
| 24 | RESULTATE_NICHT_DEFINITIV (462) | Resultate können erst hochgeladen werden, sobald die Prüfung als "definitiv" markiert wurde | Les résultats peuvent être téléchargés quand l'examen a été marquée "définitif" | *Prüfung* → « examen » (a school exam) is a false friend. Also agreement and the upload verb. | Les résultats ne pourront être transmis qu'une fois l'épreuve marquée « définitive ». |
| 25 | NENNUNG_WURDE_VERSCHOBEN (466), LOG_TITLE_nennung_verschoben (605) | Die Nennung wurde verschoben | L'engagement a été remis / Les engagements ont été modifiés. | « Remis » means postponed or handed over. The log title changes both number and meaning. | L'engagement a été déplacé. |
| 26 | KEINE_IDEALZEIT_VORHANDEN (493) | …Bitte bearbeiten Sie die Prüfung zuerst. | …Veuillez d'abord traiter cette épreuve. | *Bearbeiten* means edit; « traiter » reads as "process". | …Veuillez d'abord modifier les réglages de l'épreuve. |
| 27 | HINDERNISWERTUNGEN.A1, A2, B1, B2, JUGENDCUP (515–519) | Wertung A 1 Umgang / A 2 Umgänge / B 1 Umgang / B 2 Umgänge / A 2 Umgänge Jugendcup | Barème A 1ère manche / Barème A 2ème manche / Barème B 1ère manche / **Barème A** 2ème manche / … | "2 Umgänge" means two rounds, not "2nd round". B2 even says barème **A**. | Barème A en une manche / Barème A en deux manches / Barème B en une manche / Barème B en deux manches / Barème A en deux manches, Coupe des jeunes |
| 28 | RICHTER_IN (643) — print: every dressage judge sheet | Richter/in (Position) | Juge en (Position) | The German feminine ending "/in" was read as the preposition "in". Also copied into EquiEvents (5.2). | Juge (position) |
| 29 | VER-NR (666) — print: footer of every print (`layout.print.factory.js:142`) | Ver.-Nr (Veranstaltungs-Nr.) | Épr. n° | It is the **show** number, so the footer reads « Liste de départ … (Épr. n°: 2026.8053) ». | Manif. n° |
| 30 | ANZ_GESTARTET (668), ANZ_BEENDET (669), GESTARTET (685) — print: statistics | Anzahl gestartet / Anzahl beendet / Gestartet | Nombre de partants départ / Nombre de partants terminé / Partants départ | Word salad. | Nombre de partants / Nombre ayant terminé / Partants |
| 31 | INFO_1_KB_DERBY (754), DERBY_STRAFPUNKTE_KB (756) — print: derby control sheet | …folgende Strafsekunden vergeben / Strafsekunden | …les points de pénalisation sont attribués… / Pénalités | Derby penalties are **seconds**, not points. | …les secondes de pénalité suivantes sont attribuées : / Secondes de pénalité |
| 32 | GELAENDE_TABLE_ZEILE_19_KB (779) — print: marathon control sheet | Beifahrer:in klettert über Pferde und/oder Deichsel | Aide grimpe par-dessus les chevaux et/ou le mise des pieds à terre (les deux pieds) | The second half was pasted from another rule. The pole (*Deichsel*) is missing. | Le groom grimpe par-dessus les chevaux et/ou le timon |
| 33 | DISQUALIFIKATION (798) | Disqualifikation Pferd Elimination Fahrer | Disqualification cheval Élimination coureur | « Coureur » is a runner or racer. | Disqualification du cheval, élimination du meneur |
| 34 | 1_UHR_KB, 2_UHR_KB (804, 805) | 1. Uhr / 2. Uhr | 1ère montre / 2ème montre | A stopwatch, not a wristwatch. | 1er chronomètre / 2e chronomètre |
| 35 | GEF_ZEIT_KB (802) | Total gefahrene Zeit in **1/1000** Sekunden: | Temps du parcours en **1/100** secondes: | The two languages disagree; INFO_2_KB says 1/100 in both. Check the rule and fix one of them. | Temps total réalisé en 1/100 s : (if 1/100 is right) |
| 36 | ERROR_IMPORT_FILE_TYPE (894) | Importfile kann nur vom Typ .txt sein | Fichier illisible | The useful information is lost. | Le fichier à importer doit être au format .txt. |
| 37 | ERROR_IMPORT_VERSION_MISMATCH (895) | …Die Datensicherung hat eine neuere Version (V {{ data }}) als Ihre Installation V {{ app }}… | …La sauvegarde des données à une nouvelle version (V {{ data }}) que votre version V {{ app }}… | « à » should be « a »; « nouvelle … que » is wrong. | L'importation a échoué ! La sauvegarde provient d'une version plus récente (V {{ data }}) que votre installation (V {{ app }}). Veuillez d'abord mettre à jour Rosson. |
| 38 | ERROR_IMPORT_MASTER_FILE (896) | Import fehlgeschlagen! Die Masterdatei kann nicht importiert werden. | L'importation a échoué! C'est le fichier master. | Says something else. | L'importation a échoué ! Le fichier maître ne peut pas être importé. |
| 39 | PENDING (901) | Ausstehend | en attendant | « En attendant » means *meanwhile*. | En attente |
| 40 | NATURALPREIS (941) | Naturalpreis | Prix naturelle | Nonsense, and the wrong gender. | Prix en nature |
| 41 | TEMPO (1003) — field in km/h or m/min | Tempo | Cadence | The field is a speed. | Vitesse |
| 42 | DEUTSCH (1126) | Deutsch | Allemande | Language names are masculine. | Allemand |
| 43 | BACKUPS.LOAD (1139) | Laden | Charge | A noun instead of the verb on a button. | Charger |
| 44 | OF.CONFIRM_DELETE_CHILDREN (1190; de.js:1196) | Wollen Sie die Zuweisung wirklich löschen? *Cela peut entraîner la perte de résultats!* | Voulez-vous vraiment supprimer cette assignation?, | French drops the warning that results may be lost. The German file holds the French sentence. | Voulez-vous vraiment supprimer cette affectation ? Des résultats peuvent être perdus ! (German: … Dies kann zum Verlust von Resultaten führen!) |
| 45 | BEFORE_UPDATE, BEFORE_IMPORT (1193, 1194) | Vor Update / Vor Import | avant actualiser / avant importer | An infinitive after « avant ». | Avant la mise à jour / Avant l'importation |
| 46 | WERTUNGSPRUEFUNGEN (1202) — print: final ranking | Wertungsprüfungen | Épreuve Barème | These are the classes that count for the final. | Épreuves qualificatives |
| 47 | FINALRANGLISTEN (1199), FINALRANGLISTE (1200), FINALRANGLISTEN_EMPTY (1201), CREATE_FINALPRUEFUNG (1203) | Finalranglisten / Finalrangliste / Keine Finalranglisten vorhanden. / Finalprüfung erstellen | Classement d'une finale / Classement d'une finale / Pas du classement finales disponible. / Établir classement d'une finale | Plural lost, broken grammar; the last one means "create a final class". | Classements finaux / Classement final / Aucun classement final. / Créer une finale |
| 48 | DURCHFUEHRUNG.WARNING_RESULTAT_KEIN_START (285) | Warnung - Diese Nennung wurde als Nichtstarter markiert. Hier kann kein Resultat eingetragen werden! | Attention - Pas possible mettre un resultat pour Non-partant! | Pidgin French. | Attention : cet engagement est marqué non-partant, aucun résultat ne peut être saisi ! |
| 49 | DURCHFUEHRUNG.DELETE.ERROR (281) | Resultate konnten nicht gelöscht werden | Les résultats ne pourraient être supprimés | The conditional turns a failure into a hypothesis. | Les résultats n'ont pas pu être supprimés. |
| 50 | STARTER_EXPORT.SUCCESS, .ERROR (399, 400) | Die Starter wurden exportiert. / Das Exportieren der Starter ist fehlgeschlagen. | Les starters ont été exportés. / L'exportation du starters a échoué. | In French a « starter » is the official who starts a race (or a car part). | Les partants ont été exportés. / L'exportation des partants a échoué. |
| 51 | LOG_TEXT_pruefungen_erstellt (597) | Für Ausschreibungsprüfung Nr. {{ap_nr}} wurden {{anzahl_pruefungen}} Startlisten erstellt. | Le listes de départ {{anzahl_pruefungen}} de l'épreuve n° {{ap_nr}} ont été établies. | Garbled. | {{anzahl_pruefungen}} listes de départ ont été établies pour l'épreuve n° {{ap_nr}}. |
| 52 | LOG_TEXT_statusAenderung (594) | Prüfung {{pruefungsNr}} - {{ status \| translate }}: {{reiter}} mit {{pferd}} | **FR :)** Épreuve {{pruefungsNr}} - … | The placeholder marker is shown to users in the history. | Épreuve {{pruefungsNr}} – {{ status \| translate }} : {{reiter}} avec {{pferd}} |
| 53 | LOG_TITLE_preisgeld_angepasst (608), LOG_TEXT_preisgeld_angepasst (609), PREISGELD_AKTUALISIERT (935) | Preisgelder wurden angepasst. / Das Preisgeld wurde aktualisiert. | Les prix a été enregistrés. | Agreement; *angepasst* means adjusted, not saved. | Les prix en espèces ont été adaptés. (… pour l'épreuve n° {{pruefungs_nummer}} …) |
| 54 | TABLE.CONFIG.TABLE_CONFIG (1075) | Anzeige anpassen | Ajuster la table | A « table » is furniture; this is about the display. | Personnaliser l'affichage |
| 55 | STARTKARTEN, STARTKARTE (70, 71) | Startkarten / Startkarte | Carte de start / Cartes d'engagement | Singular and plural swapped; « start » is an anglicism; « engagement » is something else. | Cartes de départ / Carte de départ |
| 56 | ETIKETTEN_OFFIZIELLEVERSAND (67) | Etiketten Offizielleversand | Étiquettes | Truncated, so it looks like the generic label. | Étiquettes envoi aux officiels |
| 57 | TESTVERANSTALTUNG (22) | Testveranstaltung laden | Manifestation test | The action (*laden*) is lost on a button. | Charger une manifestation de test |
| 58 | DRAG_AND_DROP (21) | Dateien für den Import in dieses Feld ziehen. | Glisser les données pour l'importation dans ce champ. | *Dateien* means files, not data. | Glissez ici les fichiers à importer. |
| 59 | UEBERSICHT (34), UEBERSICHT_NENNUNGEN (129) | Übersicht / Übersicht Nennungen | Aperçu / Aperçu d'engagements | « Aperçu » means preview. | Vue d'ensemble / Vue d'ensemble des engagements |
| 60 | NENNUNGEN_NACHRUECKEN (200) | Nennungen nachrücken | Avancer un engagement | It means moving entries up from the waiting list (medium confidence). | Faire monter des engagements de la liste d'attente |
| 61 | STARTPOSITIONEN_GEAENDERT (126) | Die Startpositionen wurden getauscht | La position de départ a été changée | *Getauscht* means swapped, and it is plural. | Les positions de départ ont été interverties. |
| 62 | ERSETZEN_MIT (103) | Ersetzen mit | Remplacer avec | Calque. | Remplacer par |
| 63 | SORT_BY, SORT_ORDER (137, 138) | Sortieren nach / Reihenfolge | Trié par / Par ordre | Labels for an action, not a state. | Trier par / Ordre |
| 64 | STRAFPUNKTE_BODENTEST (1197) | Strafpunkte Bodentest | Points de pénalités Bodentest | German left in. | Pénalités test au sol (wording to check with SE) |
| 65 | GASTPFERD (325) | Gastpferd | Cheval d'hôte | Literal; « cheval d'hôte » means the host's horse. | Cheval invité (as in EquiEvents) |
| 66 | FIXE_STARTNUMMERN (217) | Fixe Start- / Gespannsnummern | Numéros de départ fixe | Drops the driving numbers; agreement. | Numéros de départ / d'attelage fixes |
| 67 | HAT_PONY_STARTBEWILLIGUNG (156) | Pony Start (config: Startbewilligung Pony) | Départ poney | It is an authorisation, not a start. | Autorisation poney |
| 68 | DISTANZEN_GELADEN (163) | Distanzen geladen | Télécharger la distance | An order instead of a status message. | Distances chargées |
| 69 | PRINT_SELECTION.AUSWAHL (1207) | Auswahl Vorlage für vorgedruckte Logos | Modèle de sélection pour les logos préimprimés | Calque ("model for selection"). | Choix du papier à logos préimprimés |
| 70 | EINSATZPLAN (1204) | Einsatzplan | Plan d'engagement | In Rosson « engagement » means *entry*, so this is ambiguous. | Planning des officiels |
| 71 | KEINE_WERTUNG_VORHANDEN (497) | Keine Wertung für diese Prüfung konfiguriert. Bitte andere Wertung auswählen. | Aucun barème a été configuré pour cette épreuve. Svp. choisir un barème. | « n' » missing; « Svp. » is too casual; « andere » is lost. | Aucun barème n'est configuré pour cette épreuve. Veuillez en choisir un autre. |
| 72 | ZEITPUNKTE_BERECHNEN (183) | Strafpunkte von Idealzeit berechnen | Calculer points pénalités du temps idéal | Telegraphic. | Calculer les pénalités par rapport au temps idéal |
| 73 | GEFAHRENE_ZEIT (208) | Gefahrene Zeit | Temps final | Not a final time. Inconsistent with GEFAHRENE_ZEIT_KB « Temps effectif » (739). | Temps réalisé (in both keys) |
| 74 | STILPRUEFUNG (947), STILPRUEFUNGEN.ERWERB (950) | Stilprüfung | Épreuves de saut avec style | Plural; the SE name is « Épreuve de style » (SE). | Épreuve de style / … en épreuves de style |
| 75 | EINLAUFPRUEFUNG (358) | Einlaufprüfung | Épreuve d'entrée | Literal (low confidence about the right term). | Épreuve préparatoire (check with SE) |
| 76 | OF.HINT_READONLY_PERSON (1175) | Sie können nur Offizielle Personen bearbeiten | Vous ne pouvez que editer des personnes officiels | « éditer » is an anglicism; agreement. | Vous ne pouvez modifier que les officiels. |
| 77 | OF.ZUWEISUNG_BEARBEITEN (1177), OF.KEINE_ZUWEISUNGEN (1181), OF.FEHLENDE_ROLLEN (1182), OF.UNBEKANNTE_PERSON (1188) | Zuweisung bearbeiten / Keine Zuweisungen / Fehlende Rollen / &lt;unbekannte Person&gt; | Editer l'assignation / Pas des assignations / Rôles absent / &lt;Persone inconnu&gt; | Typos, agreement, anglicism. | Modifier l'affectation / Aucune affectation / Rôles manquants / &lt;Personne inconnue&gt; |
| 78 | VERSCHIEBEN_IN (469) | Verschieben in | Changer d'épreuve dans | Clumsy and redundant. | Déplacer vers |
| 79 | TOTAL_NENNGELDER (885) — print: letters to riders (`serienbrief-worker.print.service.js:114`) | Total Nenngelder: | Total des engagements: | Reads as a *number of entries*, not an amount. | Total des finances d'engagement : |

### 2.2 Terminology and inconsistencies

| # | Keys (fr.js lines) | German | Rosson French | Why | Proposed French |
|---|---|---|---|---|---|
| 1 | OTHER_NENNUNGEN_PAAR (107), GWP_PAAR (336, 1066, 1091), GEWINNPUNKTE_REITER_PFERD_PAAR (823), EQUIPEN_STARTKARTE.PAAR (837) | Paar | paire | A rider-and-horse pair is a « couple ». | couple (e.g. « Engagements du couple : ») |
| 2 | RANGLISTE, RANGLISTE_PROV, RANGLISTE_AKTUALISIEREN(_SUCCESS), INFO_RANGLISTE (366–370) vs RANGLISTE_DRUCKEN.TITLE (373), REIHENFOLGE_NACH_RANGLISTE.* (1166–1172) | Rangliste | « liste de résultat » and « classement » | Same word, two translations. « Liste de résultat » also lacks the plural. | Classement everywhere (« Classement provisoire », « Actualiser le classement ») |
| 3 | STRAFPUNKTE (413), TOTAL_1_2_STRAFPUNKTE_KB (747), STRAFPUNKTE_DRESSUR (1195), STRAFPUNKTE_PRO_HINDERNIS (1196); STRAFP (827), DERBY/GELAENDE_STRAFPUNKTE_KB (756, 757); STRAFPUNKTE_KB (746); INFO_1_KB_GELAENDE (753), TOTAL_STRAFPUNKTE_KB (799) | Strafpunkte | « Points de pénalités », « Pénalités », « pénalités », « points de pénalisation » | Four forms. « points de pénalités » should be singular *pénalité*. SE's own dressage texts say « points de pénalisation ». | Pick one: « points de pénalité » (short form « pénalités ») or SE's « points de pénalisation ». |
| 4 | STARTNUMMER (330), START_NR (639), TABLE.CONFIG.STARTNUMMER (1077), EQUIPEN_STARTKARTE.STARTNUMMER (843), STARTLISTE_START_NR (698), TABLE.HEAD.STARTNUMMER (1052) | Startnummer / Start-Nr. | « Numéro de start », « No de start », « Départ n° » | « start » is an anglicism; several forms. | N° de départ |
| 5 | GWP (192), GWP_SHORT (193), GWP_REITER/PFERD/PAAR (334–336), TABLE.HEAD.GWP_* (1066–1068), TABLE.CONFIG.GWP_* (1091–1093), GEWINNPUNKTE_REITER_PFERD_PAAR (823) | Gewinnpunkte (GWP) | « Somme de points », « SOP », « SoP-… », « … SoP » | *Gewinnpunkte* are points won, not a sum. Three spellings. EquiEvents says « points de gain ». | Use Swiss Equestrian's French term (to verify: probably « points de gain », abbreviation to confirm), and one form only. |
| 6 | STARTLISTE_START_NR_SIEGERRUNDE (700), STARTLISTE_SIEGERRUNDE (702) | Siegerrunde | tour des vainqueurs | SE's own scoring tables say « round des gagnants » (SE). | round des gagnants (to match SE and EquiEvents) |
| 7 | PLATZPFLEGE (703), PLATZPFLEGE_NACH (201) | Platzpflege | Entretien de la place | « place » is understood in Switzerland, but the show-jumping term is the arena (« piste »). | Entretien de la piste (as in EquiEvents) |
| 8 | GELAENDE_TABLE_ZEILE_1_KB (761) vs ZEILE_5 (765), ZEILE_11 (771), DERBY_TABLE_ZEILE_1 (781), ZEILE_14 (794, « par un aide ») | Beifahrer:in | « groom » in one line, « aide » (with both genders) in the others | Inconsistent. FEI French driving rules say « groom ». | groom everywhere (« par le groom ») |
| 9 | GELAENDE_TABLE_ZEILE_0_KB (760) vs ZEILE_1 (761), DERBY_TABLE_ZEILE_1 (781) | abwerfbares Element | « élément tombant » vs « élément renversable » | Tautology (« chute d'un élément tombant »); inconsistent. | Chute d'un élément renversable |
| 10 | ZUSCHLAG_KB (734), SPURBREITE_KB (735), WAGENBREITE (333) vs WAGENBREITE_KB (733) | Zuschlag / Spurbreite / Wagenbreite | « Ecartement » / « Largeur » / « Largeur voiture » vs « Largeur de la voiture » | In cones driving, the gap is the track width plus an allowance. Medium confidence on the exact FEI French wording. | Supplément / Largeur de voie / Largeur de la voiture |
| 11 | WAGON_NR (715) | Wagon Nr. | Wagon n° | In driving the carriage is a « voiture ». The German is odd too. | Voiture n° |
| 12 | LEKTIONEN (447), DR_AUFGABEN (647) | Lektionen / Aufgaben | « Reprise » / « Figures » | Two words for the movements column. SE's own test texts title the table « Reprise » or « Mouvements de la reprise » (SE), so « Reprise » is defensible but unclear. EquiEvents says « Mouvement ». | Mouvements (in both keys) |
| 13 | ZEITMESSUNG_EXPORT_TITEL (392) | Export für Zeitmessanlage: | Export pour le chronométreur: | *Zeitmessanlage* is the timing system, not the person. | Export pour le chronométrage : |
| 14 | STILPRUEFUNGEN.NOTENSKALA (954) | Notenskala | Barème des notes | In the rest of Rosson « barème » is the jumping scoring table. Optional: Gestion Concours' style sheet also says « Barème des notes ». | Échelle des notes |
| 15 | STILPRUEFUNGEN.KORR_EINREITEN (975) | Korrektes Einreiten, Pferd am Zügel | entrée en piste correcte, cheval à la rêne | « Am Zügel » is « sur la main » in French equitation. Optional, because it is Gestion Concours' wording. | Entrée en piste correcte, cheval sur la main |
| 16 | PROZENT (191), RESULTAT_IN_PROZENT (658) | Prozent | Pourcent / Résultat en pourcent | « pourcent » is not a noun in French. Also copied into EquiEvents. | Pourcentage / Résultat en pourcentage |
| 17 | RANGPUNKTE (190) | Rangpunkte | Points rang | Missing preposition. | Points de classement |
| 18 | PREISGELD (940), PREISGELD_DEFINIEREN (934), MIN_PREISGELD (936), PREISGELD_ERSTER_PLATZ (938) | Preisgeld | « Prix », « Définir le prix », « Min. prix » | « Prix » alone also means price. | Prix en espèces / Définir les prix en espèces / Prix minimum |
| 19 | SAVE (10) vs « enregistrez » (142), « enregistré » (498, 527) | Speichern | Sauvegarder / enregistrer | Two verbs. | Enregistrer |
| 20 | DELETE (26) « Supprimer » vs DELETE_STARTLISTEN (165), BESTEHENDE_RESULTATE… (227), NENNUNG_LOESCHEN (445), WERTUNG_GELOESCHT (499), PROGRAMM_WILL_BE_DELETED (524) … « Effacer » | Löschen | Supprimer / effacer | Two verbs. | Supprimer |
| 21 | CREATE_STARTLISTEN (85), PRUEFUNGEN_ERSTELLT (87), ERSTELLEN (492), PROGRAMM_ERSTELLT (528) « établir » vs EQUIPEN.CREATE (930), ETIKETTEN.NEUE_VORLAGE (1148) « créer » | Erstellen | établir / créer | « Établir » fits lists (« établir les listes de départ ») but not programmes or buttons. | Créer (keep « établir » for start lists) |
| 22 | FEHLER_BEIM_LADEN_404 (475) | Veranstaltung | « L'événement » (plus a stray space in « n' a ») | Everywhere else it is « manifestation ». | La manifestation avec le code d'accès « {{ accessKey }} » est introuvable. |

### 2.3 Grammar, agreement, typos

| Key (fr.js line) | Rosson French | Proposed French |
|---|---|---|
| POSITION_ADRESSE (18) | Position d'adresse | Position de l'adresse |
| ZWISCHENRESULTATE_HOCHLADEN (39) | Télécharger les résultats partielles | Transmettre les résultats intermédiaires |
| STARTLISTE_HOCHGELADEN (41) | …ont été téléchargés. | Les listes de départ ont été transmises. |
| VERANSTALTUNG_ABSCHLIESSEN (43) | clore la manifestation | Clôturer la manifestation |
| VERANSTALTUNG_HOCHGELADEN (45) | La manifestation est été téléchargé. | La manifestation a été transmise. |
| ABMELDUNGEN (48) | Désinscrit | Désinscriptions |
| FALSCHMELDUNGEN (49) | Engagements erroné | Engagements erronés |
| ZURUECKGEWIESENE (50) | Refusé | Refusés |
| PFERDEVERWALTUNG (53) | Gestion du cheval | Gestion des chevaux |
| STARTLISTE_LITE (62) | Liste de départ petit | Liste de départ abrégée |
| TAGES_STARTLISTE_HEADER (64) — printed before a date | Liste de départ du jour | Liste de départ du |
| DELETED_STARTLISTEN (92) | …ont été supprimés. | …ont été supprimées. |
| DELETED_STARTLISTEN_FAILED (93) | Suppression de la liste de départ sur info ne fonctionnait pas. | La suppression des listes de départ sur info n'a pas fonctionné. |
| FAHRERWECHSEL (97) | Changement meneur | Changement de meneur |
| REIHENFOLGE_ANGEPASST (139) | L'ordre a été changée | L'ordre a été modifié |
| WARNING_RESULTATE_PRO_RICHTER (142) | L'épreuve sélectionné… cliquez sur "Défausser" | L'épreuve sélectionnée… cliquez sur « Ignorer les modifications » |
| WARNING_RICHTERBLAETTER (144) | …peut durer bien {{ minutes }} minutes… | …peut prendre jusqu'à {{ minutes }} minutes… |
| ERROR_RICHTERBLAETTER (145) | feuilles de juge n'a pas pu être créé | Les feuilles de juge n'ont pas pu être créées. |
| DISTANZEN_BERECHNEN (162) | Calculer la distance | Calculer les distances |
| STARTZEITEN (175) | Heure de départ | Heures de départ |
| MAX_ZEIT_BERECHNEN (182) | Calculer du temps maximal | Calculer le temps maximal |
| KEINE_PRUEFUNG (309) | Épreuves ne crée pas | Aucune épreuve créée |
| NEUE_NENNUNG (312), LOG_TITLE_neue_nennung (590) | Nouveau Engagements / Nouveau engagement. | Nouvel engagement |
| STARTPLAETZE (350) | Place de départ | Places de départ |
| TEILRESULTATE (371) | Résultats partielles | Résultats partiels |
| STREICHRESULTAT_initial_2 (423) | Résultat biffé, 2ème manche | Résultat biffé 2e manche (no comma, like the 1st) |
| HERLEITUNG_B_KUER (427) | Notes artistiques (B: | Notes artistiques (B) : |
| WERTUNG_UPDATE_SUCCESS (503), PROGRAMM_UPDATE_SUCCESS (531), ETIKETTEN.VORLAGE_AKTUALISIERT (1161) | …a été actualisée | …a été actualisé |
| PROGRAMM_WAEHLEN (523) | …ou créez un nouveau. | …ou en créer un nouveau. |
| DURCHFUEHRUNG.DELETE.WARNING (283) | Cela efface tous les résultats. Cela ne peut pas être annulée! | Tous les résultats seront supprimés. Cette action est irréversible ! |
| REIHENFOLGE_UEBERTRAGEN.CONTENT (290) | …pour cette engagement à tous les épreuves partiels? | …de cet engagement à toutes les épreuves partielles ? |
| REIHENFOLGE_UEBERTRAGEN.YES (291) | Pour tous les épreuves partiels | Pour toutes les épreuves partielles |
| LOG_TEXT_gwpwechsel (573) | …avant ({{gwp}}) ensuite ({{gwpNew}}) | …avant ({{gwp}}), nouveau ({{gwpNew}}) |
| LOG_TITLE_veranstaltung_geladen (581) | La manifestation a été téléchargée. | La manifestation a été chargée. |
| LOG_TEXT_folgepruefungen_erstellt (600) | Les lists de départ… | Les listes de départ… |
| LOG_TEXT_adress_wechsel (612) | …a changé: Avant (…) ensuite (…) | …a changé : avant (…), nouvelle (…) |
| LOG_TITLE_begleitperson_angepasst (617) | Accompagnateur a été adapté | L'accompagnateur a été modifié |
| LOG_TITLE_nenngeld_angepasst (620) | Finance d'engagement a été adapté | La finance d'engagement a été modifiée |
| falschnennung (628) | Erreur d'engagement | Engagement erroné (to match FALSCHMELDUNGEN) |
| GELAENDE_BESCHREIBUNG_KB (755) | Déscription | Description |
| GELAENDE_TABLE_ZEILE_4_KB (764) | …manquants ou déchirés… et pas corrigé… | …manquants ou cassés… et non corrigés… |
| EQUIPEN_STARTKARTE.ZUSCHL (841) | Pénaltiés | Pénalités |
| SUCCESS_DOWNLOAD (892) | Téléchargées avec succès | Téléchargement réussi |
| ERROR_IMAGE_FILE_TYPE (897) | Image peut être .png ou de type .jpeg | L'image doit être au format .png ou .jpeg. |
| IMAGES_UPLOAD_SUCCESS (898) | L'image a été ajouté avec succès | L'image a été ajoutée. |
| UPDATE.ERROR (909) | La mise à jour à échoué. | La mise à jour a échoué. Réessayez plus tard. |
| EQUIPEN.TITLE (927) | Équipe | Équipes |
| STILPRUEFUNGEN.LIZENZKANDIDATIN (951) | Candidat/e | Candidat/e à la licence |
| STRAFPUNKTE_PRO_HINDERNIS (1196) | Points de pénalités obstacle | Pénalités par obstacle |
| DATENBANK (1102), DEV_MODE (1103) | Données / Développement | Base de données / Mode développeur |
| ETIKETTEN.MARGIN, PAGE_MARGIN_TOP, PADDING, CENTER (1151–1156) | Marge étiquettes / Marge page / Écartement / Centrer | Espacement entre étiquettes / Marge supérieure / Retrait du texte / Centré |
| PRINT_SELECTION.CREATE_PDF (1208) | CREER PDF | CRÉER LE PDF |

### 2.4 Missing in French (German shows instead)

`ALTE_ROSSON_VERSION`, `ALTE_ROSSON_VERSION_UPDATE` (de.js:25–26), `NENNUNGEN_REITER` (de.js:105), `DATUM_ZEIT` (de.js:351), `WAGON` (de.js:721), `EQUIPEN_STARTKARTE['ZUSCHL.']` and `BACKUPS.VERSION`.

Proposed French, in the same order:

- « Vous utilisez une version de Rosson plus ancienne que celle qui a créé ces données. »
- « Veuillez mettre à jour Rosson avant de continuer. »
- « Engagements du cavalier : »
- « Date/heure »
- « Voiture »
- « Pén. »
- « Version »

### 2.5 Texts hard-coded in Rosson's prints

| File:line | What | Problem | Fix |
|---|---|---|---|
| `startliste.print.service.js:419–420` | `'Punkte'`, `'Zeit'` | German column heads printed on French (and Italian) team start lists for the *Siegerrunde*. | Use `t('PUNKTE')` and `t('ZEIT')`, with ZEIT corrected to « Temps ». |
| `layout.print.factory.js:142` | footer `${title} (${t('VER-NR')}: …)` | Prints « Épr. n° » before the show number on every page (see 2.1 #29). | Fix VER-NR. |
| `layout.print.factory.js:127` | `locales = lang === 'de' ? 'de-DE' : 'fr-FR'` | Every non-German print gets French dates, including Italian. | Map `it` to `it-CH`, `fr` to `fr-CH`. |
| `layout.print.factory.js:133` | `'Uhr'` only for German | French prints get « 14:05 » with no unit. That is acceptable; « 14 h 05 » would be more French. | Optional |
| `layout.print.factory.js:363` | `'Postfach 726, Papiermühlestrasse 40 H, CH-3000 Bern 22'` | Swiss Equestrian's address is German on French and Italian headers. | « Case postale 726 » / « Casella postale 726 » (the street name stays) |
| `fahren-rangliste.print.service.js:267` | `t('ZEIT') + ' ' + t('HN')` | Prints « Heure Os » (see 2.1 #19–20). | Fix both keys: « Temps obst. » |
| `ranglisten.print.service.js:714–720` | weekday and month through `t()` | French is fine. Italian has none of these keys, so Italian rankings print German day and month names. | Add the Italian keys. |
| `startkarten.print.service.js:248` (commented out) | `['Strafp.', 'Pénaltiés']` | Typo in dead code. | Delete it, or fix the typo if it is revived. |

### 2.6 Typography (as a class)

Swiss French needs a (narrow) non-breaking space before « : ; ? ! » and inside « ». Rosson never has one. The problems, with counts:

- No space at all before « ? » in 12 values (« Êtes-vous sûr? », « Voulez-vous continuer? ») and before « ! » in 17.
- No space before « : » in about 49 values (« Engagements cavalier: », « Temps accordé: »).
- Straight quotes `"définitif"` in 10 values, « » in only one.
- Capitals without accents in about 15 values: « A la fin », « Etes-vous », « Ecraser », « Epreuve de terrain », « Editer », « CREER ».
- « 1ère / 2ème » in 20 values. The norm is « 1re / 2e ».
- « No » / « no. » in 6 values next to « N° ».
- Straight `'` in 108 values and `’` in 51.

---

## 3. Rosson Italian

### 3.1 Coverage

| | Keys | Share |
|---|---|---|
| German keys (leaf values, nested ones counted) | 1,021 | 100 % |
| Translated (active in `it.js`) | **53** | 5.2 % |
| — of which developer error log (`ERROR_LOG.*`) | 19 | |
| — of which only `{{veranstaltung}}` | 1 | |
| Commented out, German text with an `IT :)` marker | 436 | 42.7 % |
| Commented out, German text without a marker | 228 | 22.3 % |
| Commented out, **French** text (e.g. `STARTLISTE_EXPORT` it.js:74, the whole style-sheet block it.js:733–772) | 21 | 2.1 % |
| Not in `it.js` at all (e.g. HINZUFUEGEN, INFO_MIGRATION, VERANSTALTUNG, UMGANG, DRESSURPUNKTE, EINLAUFPRUEFUNG, all ZEITMESSUNG_*) | 283 | 27.7 % |

Three commented lines even carry the French marker (`FR :)`, it.js:62, 335, 340).

Because `fallbackLanguage('de')` is set (de.js:1225), an Italian user sees German for about 95 % of the interface and the prints. The only Italian is on the dressage judge sheet labels, a few address fields and the error log.

Prints add two more problems:

- Dates are formatted `fr-FR` for every non-German language (`layout.print.factory.js:127`).
- Weekday and month names come from translation keys that Italian lacks, so they print in German.

In short, Rosson has no Italian worth keeping. EquiEvents should translate Italian from scratch using the glossary in section 4, not start from `it.js`.

### 3.2 Flagged Italian values

| Key (it.js line) | German | Rosson Italian | Why | Proposed Italian |
|---|---|---|---|---|
| MITBESITZER (210) | Mitbesitzer | Comproprietaria | Feminine only. | Comproprietario/a |
| ORT (216) | Ort (address) | Luogo | In an address the field is the locality. | Località |
| LEKTIONEN (391) | Lektionen | Ripesa | Typo for « Ripresa ». SE's own test texts title the table « Ripresa » or « Movimenti nella ripresa » (SE). | Movimenti (or at least « Ripresa ») |
| POSITION (395) | Position (judge at C, E, H…) | Postazione | « Postazione » is possible, but « posizione » is the usual word (low confidence). | Posizione |
| PRUEFUNGSNR (538) | Prüfungs-Nr. | Prova n° | SE Italian calls a class « gara » (« Gara ai punti », « Gara speciale »). Italian abbreviates "numero" as « n. ». | Gara n. |
| START_NR (539) | Start-Nr. | N°di partenza | Missing space; « N° » is not Italian usage. | N. di partenza |
| RICHTER_IN (542) | Richter/in (Position) | Giudice | « (Position) » is lost, so the sheet no longer says where the judge sits. | Giudice (posizione) |
| NOTE (543) | Note (a mark) | Note | False friend: Italian « note » means notes or remarks. A dressage mark is a « voto ». | Voto |
| DR_LEITGEDANKEN (547) | Leitgedanken | Linee direttrici | Italian FEI dressage sheets say « Idee direttrici » (medium confidence). | Idee direttrici |
| TOTAL (551) | Total | Totale punti | Adds "points" to a generic key that is used for other totals too. | Totale |
| UEBERTRAG_VON_SEITE, UEBERTRAG_AUF_SEITE (554, 555) | Übertrag von / auf Seite | Segue da pagina / Continua a pagina | These are totals carried over, not text that continues. | Riporto da pagina / Riporto a pagina |
| SCHWEIZERISCHER_VERBAND_FUER_PFERDESPORT (556) | Swiss Equestrian | Federazione Svizzera Sport Equestri | The old name (FSSE). German and French already use the brand. | Swiss Equestrian |

The other 41 active values are acceptable: `Cavallo`, `Osservazioni`, `Punti di penalità`, `Impressione generale`, `Movimenti`, `Criteri di valutazione`, `Risultato in percentuale`, the error log, and so on.

---

**Italian in EquiEvents (2026-11-14).** The app (`app/src/i18n-it.ts`), the prints and exports
(`crates/office/src/italian.rs`, each French print text with its Italian; a test prints everything
in Italian and fails on any text left in French) and Swiss Equestrian's own Italian for roles and
dressage tests (German where it has none). A class is « gara », as Swiss Equestrian's Italian.
Checked against Swiss Equestrian's « Regolamento di salto (RS) & Direttive (DRS) », status
01.01.2026 (info.swiss-equestrian.ch, document 319, Italian), on 2026-11-14:
- confirmed: gara, spareggio, winning round (« giro del vincitore »), percorso iniziale / percorso
  base, cavaliere/amazzone, concorrenti, partenti, classificati, tassa di iscrizione, voto (style
  classes), giudice di stile, gara di stile, squadra, tempo ideale, tempo accordato, lista / ordine /
  numero di partenza, punti di penalità, cambio di cavaliere, Presidente della Giuria,
  costruttore/costruttrice di percorsi, campo prova, ex aequo;
- changed to follow it: Gewinnpunkte « punti di vincita » (abbreviated « PV »: the rules give no
  abbreviation); Rang « rango » (« 1° Rango »), not « posizione »; « giudice alla riviera »;
  hors concours « fuori gara » (HC); « cavaliere/amazzone », not « cavaliere/a »; « tassa di
  iscrizione »; aufgegeben « ritirato » (« si ritira »), verzichtet « rinuncia »;
- not in that document (still proposals): cartellino di partenza (Startkarte), segnalamento
  (Signalement), annullata (abgemeldet), scartato (a team's dropped result), esercitazione (a
  rehearsal of a show), gara pubblicata (an Ausschreibung is « proposizioni »), and the dressage and
  driving terms (other rules).

## 4. Recommended glossary: German → French → Italian

**Confidence**:

- **SE** — the term appears in Swiss Equestrian's own master data in the corpus (role names, scoring tables, reasons, dressage test texts).
- **high** — established federation usage.
- **med.** — likely right, worth confirming.
- **low** — check with Swiss Equestrian or the rules before using.

When French usage in Rosson or EquiEvents differs, the note says so.

### Shows, people, horses

| German | French | Italian | Confidence / note |
|---|---|---|---|
| Turnier / Veranstaltung | concours / manifestation | concorso / manifestazione | SE (« DT du concours », « Veterinario del concorso ») |
| Ausschreibung | avant-programme | avamprogramma | high FR, med. IT |
| ausgeschriebene Prüfung | épreuve de l'avant-programme | gara dell'avamprogramma | med. (EquiEvents says « épreuve publiée ») |
| Prüfung | épreuve | gara (also: prova) | SE uses « gara » in Italian (« Gara ai punti ») |
| Teilprüfung | épreuve partielle | prova parziale | med. |
| Folgeprüfung | épreuve consécutive | gara successiva | med. |
| Abteilung (of a divided class) | série | serie | low: Rosson says « série », EquiEvents « partie »; check SE rules |
| Kategorie | catégorie | categoria | SE |
| Stufe | degré / niveau | livello | low: Rosson « degré », EquiEvents « niveau »; pick one with SE |
| Springen / Dressur / Fahren / Concours Complet / Voltige | saut / dressage / attelage / concours complet / voltige | salto / dressage / attacchi / concorso completo / volteggio | SE |
| Turniersekretariat (not « Turnierbüro ») | secrétariat de concours (not « bureau ») | segreteria del concorso | decided by the user, 2026-11-15 |
| OK-Präsident | président du CO | presidente del CO | SE |
| Offizielle | officiels | ufficiali di gara | high FR, med. IT |
| Richter/in | juge | giudice | SE |
| Jury-Präsident/in | président/e du jury | presidente di giuria | SE |
| Präsident/in der Prüfung | président/e de l'épreuve | presidente della gara | SE FR, med. IT |
| Stilrichter/in | juge de style | giudice di stile | SE |
| Richter/in Wassergraben | juge de rivière | giudice della riviera | SE FR, low IT |
| Abreitplatz | place d'entraînement / place d'échauffement | campo prova | SE (both French forms occur) |
| Technische:r Delegierte:r | délégué/e technique (DT) | delegato/a tecnico/a (DT) | SE |
| Parcoursbauer/in | constructeur/trice de parcours | costruttore/trice di percorsi | SE |
| Zeitmessung / Zeitmesser/in | chronométrage / chronométreur/euse | cronometraggio / cronometrista | SE (chronométrage) |
| Turniertierarzt | vétérinaire de concours | veterinario del concorso | SE |
| Reiter/in | cavalier / cavalière | cavaliere / amazzone | high |
| Pferd / Pony | cheval / poney | cavallo / pony | high |
| Paar (Reiter + Pferd) | couple | binomio | high (« paire » is wrong) |
| Besitzer/in / Züchter/in | propriétaire / éleveur, éleveuse | proprietario/a / allevatore/trice | high |
| Abstammung (V × MV) | origines (père × père de la mère) | origini (padre × padre della madre) | high FR, med. IT |
| Gastpferd | cheval invité | cavallo ospite | low |
| Begleitperson / Führperson | accompagnateur/trice | accompagnatore/trice | SE |
| Equipe / Mannschaft | équipe | squadra | high |
| Verein | société | società | SE (« Entraîneur de société », « Allenatore di società ») |
| Lizenz / Brevet / Pass-Nr. | licence / brevet / n° de passeport | licenza / brevetto / n. di passaporto | SE (brevet, brevetto) |

### Entries and start lists

| German | French | Italian | Confidence / note |
|---|---|---|---|
| Nennung | inscription (EquiEvents) / engagement (Rosson) | iscrizione | high; use one French word throughout |
| Nachnennung | inscription tardive | iscrizione tardiva | med. |
| abgemeldet | désinscrit | iscrizione annullata | med. (« ritirato » is taken by *verzichtet*) |
| Falschnennung | inscription erronée | iscrizione errata | med. |
| zurückgewiesen | refusé | respinto | med. |
| Warteliste | liste d'attente | lista d'attesa | high |
| Nenngeld | finance d'inscription | tassa d'iscrizione | med. (Swiss French « finance »; « quota d'iscrizione » also used) |
| Startliste | liste de départ | lista di partenza | high |
| Startreihenfolge / auslosen | ordre de départ / tirer au sort | ordine di partenza / sorteggiare | high |
| Startnummer | n° de départ | numero di partenza | high |
| Startzeit | heure de départ | orario di partenza | high |
| Starter / Nichtstarter (NS) | partant / non-partant (NP) | partente / non partente (NP) | high FR, med. IT |
| hors concours (hc) | hors concours (HC) | fuori concorso (HC) | med. IT (« hors concours » is also used in Italian) |
| Zeitplan | horaire | orario | high |
| Startkarte | carte de départ | cartellino di partenza | low IT |
| Richterblatt | feuille de juge | foglio del giudice | med. FR, low IT |

### Results

| German | French | Italian | Confidence / note |
|---|---|---|---|
| Rangliste | classement | classifica | high |
| Rang / klassiert | rang / classé | posizione / classificato | SE (« non classificato ») |
| in Wertung (status: not out) | en compétition | in gara | EquiEvents (user, 2026-11-15): never « classé » for it, which is the placed riders (prizes) |
| ausgeschieden | éliminé | eliminato | SE |
| aufgegeben | abandon | abbandonato | SE |
| verzichtet | retiré | ritirato | SE (EquiEvents already matches) |
| disqualifiziert | disqualifié | squalificato | SE IT |
| definitiv / provisorisch | définitif / provisoire | definitivo / provvisorio | high |
| Preisgeld | prix en espèces | premio in denaro (montepremi for the total) | med. |
| Naturalpreis | prix en nature | premio in natura | high |
| Gewinnpunkte (GWP) | points de gain (?) | punti vinti (?) | **low**: confirm Swiss Equestrian's French and Italian terms and abbreviations |

### Show jumping

| German | French | Italian | Confidence / note |
|---|---|---|---|
| Wertung (Springen) | barème | tabella | SE |
| Wertung A mit Zeitmessung | barème A au chronomètre | tabella A a tempo (con cronometro) | SE |
| Punktespringen / Zeitspringen | épreuve aux points / épreuve contre la montre | gara ai punti / gara contro il tempo | SE |
| Stilprüfung | épreuve de style | prova di stile | SE FR. Low IT: SE writes « gara di precisione », which in Italian usage means a class without time; confirm |
| Einsteigerprüfung | épreuve d'initiation | gara introduttiva | SE |
| Umgang | manche | manche | SE (« 2 manches »; Italian also uses *manche*) |
| Initialparcours | parcours initial (p.i.) | percorso base | SE FR, med. IT |
| Stechen | barrage | spareggio | SE (*barrage* is also current in Italian) |
| Siegerrunde | round des gagnants | winning round | SE wording in both. Rosson French says « tour des vainqueurs ». |
| Idealzeit | temps idéal | tempo ideale | SE |
| erlaubte Zeit / Höchstzeit | temps accordé / temps limite | tempo accordato / tempo limite | high FR, med. IT |
| Strafpunkte | points de pénalité (SE: « points de pénalisation ») | punti di penalità | SE |
| Abwurf / Verweigerung / Sturz | barre tombée / refus / chute | abbattimento / rifiuto / caduta | high FR, med. IT |
| Zeitfehler | pénalités de temps | penalità di tempo | med. |
| Platzpflege | entretien de la piste | manutenzione del campo | med. |

### Dressage

| German | French | Italian | Confidence / note |
|---|---|---|---|
| Dressurprüfung / Programm / Aufgabe | reprise (« Reprise de dressage FB 04/60 ») | ripresa (« Ripresa di dressage FB 04/60 ») | SE. Rosson and EquiEvents say « programme »; both are understood, but SE's own texts say « reprise ». |
| Kür | reprise libre | ripresa libera | SE |
| Lektion | mouvement | movimento | SE (« Mouvements de la reprise », « Movimenti nella ripresa ») |
| Note | note | voto | high |
| Koeffizient | coefficient | coefficiente | high |
| Leitgedanken | idées directrices | idee direttrici | high FR, med. IT |
| Gesamteindruck | impression générale | impressione generale | SE |
| Programmfehler | erreur de parcours | errore di percorso | SE (« Troisième erreur » / « Terzo errore ») |
| Viereck | carré | rettangolo | SE (« Carré 20 x 60m », « Rettangolo 20 x 60m ») |

### Driving

| German | French | Italian | Confidence / note |
|---|---|---|---|
| Gespann | attelage | equipaggio | high FR, med. IT |
| Fahrer/in | meneur / meneuse | driver (guidatore) | high FR, low IT |
| Beifahrer/in | groom | groom | high |
| Wagen | voiture | carrozza | med. |
| Kegelfahren | maniabilité | prova coni (maneggevolezza) | high FR, med. IT |
| Gelände (Fahren) | marathon (épreuve de terrain) | maratona | med. |
| Hindernis | obstacle | ostacolo | high |
| Spurbreite | largeur de voie | carreggiata | med. |
| Derby | derby | derby | SE |

---

## 5. EquiEvents' own French

Overall the French is idiomatic and close to the federation's wording: *épreuve, barème, barrage, non-partant, juge de style, finance d'inscription, cheval invité, round des gagnants* (as SE), *retiré* for *verzichtet* (as SE). The items below are the real problems found.

**Status (2026-11-14).** Applied: every row of 5.1–5.3 below, except those waiting for a
decision. Also applied: 5.4, with a narrow no-break space before « : ; ? ! » and inside « » in the
app's French (`typeset` in `i18n.tsx`), and typographic apostrophes in the prints' own French.
Decided (P. Burkhard, 2026-11-14): *Gewinnpunkte* « Somme de points », « SoP »; « Épreuve
publiée » stays; the inclusive « collaborateur·trice »; a class divided before it runs is in
« séries » (German « Serien », as Rosson; no longer « parties » / « Abteilungen »).

### 5.1 `app/src/i18n.tsx` (line in the `fr` dictionary)

| Key (line) | German | EquiEvents French | Why | Proposed French |
|---|---|---|---|---|
| starts_in (47) | Startet in | Part dans | Reads like "leaves in (5 minutes)". The column lists the rider's classes. | Inscrit en (or Épreuves) |
| published_by_name (78) | Ausgeloste Reihenfolge. Veröffentlicht nach Namen, solange… | Ordre tiré au sort. Publiée par ordre alphabétique tant qu'… | « Publiée » has no subject, and « ordre » is masculine. | Ordre tiré au sort. La liste est publiée par ordre alphabétique tant qu'il n'y a pas de numéros de départ. |
| legend_move (111) | bewegen | se déplacer | The reflexive means moving oneself. | déplacer |
| timekeeper_out, timekeeper_in, timekeeper_file (143–145) | Liste für die Zeitmessung… / Resultate der Zeitmessung… / Datei der Zeitmessung | …chronométreur… | The file comes from the timing system, not from a person. | Liste pour le chronométrage… / Résultats du chronométrage… / Fichier du chronométrage |
| adopt_ranks (147) | Ränge übernehmen | Reprendre ses rangs | Sounds like "fall back in line". « ses » is ambiguous. | Reprendre les rangs |
| round (152); prints.rs:1173–1175 and `words` round_1/round_2 (93–94) | Umgang | Parcours / 1er parcours / 2e parcours | SE's scoring tables say « 2 manches ». « Parcours » collides with « parcours initial » before a barrage. | Manche / 1re manche / 2e manche (keep « parcours initial » for one round plus barrage) |
| combined_classes (182) | Prüfungen (in der gerittenen Reihenfolge) | Épreuves (dans l'ordre courues) | Ungrammatical. | Épreuves (dans l'ordre où elles ont été courues) |
| number_show (201), numbers_given (202) | Startnummern vergeben / Nummern vergeben | Donner les numéros / numéros donnés | Colloquial. change_number (90) already says « attribués ». | Attribuer les numéros de départ / numéros attribués |
| order_points (281); prints.rs:356 « Pts gain » | Gewinnpunkte (GWP) | points de gain (GWP) | German abbreviation in a French label. The French term itself is to be confirmed (see glossary). | points de gain (PG?), after checking with SE |
| order_ranking_hint (287) | …ohne Resultat dort: am Ende. | …sans résultat là-bas : à la fin. | Calque of *dort*. | …sans résultat dans cette épreuve : à la fin. |
| following (291), change_set_up_following (294) | Folgeprüfung… / Folgeprüfung vorbereitet | Suite de… / épreuve préparée comme une autre | Unclear. | Épreuve consécutive… / épreuve préparée d'après une autre |
| sections (73), following_as (292), following_hint (293), sections_change (320), sections_parts (322), sections_apply (334), sections_empty (336), sections_apart (337), divide_into (350) | Abteilung(en) | partie(s) | Generic word; Rosson says « série ». Confirm the Swiss Equestrian French term for *Abteilung*, then use it. | série(s), if SE confirms |
| programme_lasts (313) vs programme (123), reference_tests (556) | Programm: | reprise : | « Reprise » here but « Programme » elsewhere. | Pick one. SE's own texts use « reprise ». |
| max_starters (314) | Höchstens Starter | Partants au plus | Odd word order for a label. | Nombre max. de partants |
| rule_club (326), sections_search (332); prints.rs:1256 « Club » vs club (44) « Société » | Verein | club / Société | Two words for *Verein*. SE says « société ». | Par société / société |
| team_not_starting (354) | Die ganze Mannschaft startet nicht | Toute l'équipe ne part pas | Ambiguous: it also reads "not all of the team starts". | L'équipe entière est non-partante |
| remove_start_lists_hint (383) | …Schon gesendete Listen zuerst zurückziehen (Bildschirm Swiss Equestrian). | …Des listes déjà envoyées se retirent d'abord (écran Swiss Equestrian). | Odd passive (*se retirent*). | …Retirez d'abord les listes déjà envoyées (écran Swiss Equestrian). |
| print_team_cards (308) vs prints.rs:1352 / 1366 | Equipen-Startkarten | Cartes des équipes / Cartes de départ des équipes | Menu and print title differ. | Cartes de départ des équipes |
| print_style_sheets (394); prints.rs:1508 | Stilrichterblätter | Feuilles de style | Also means CSS style sheets. Keep it only if it is Gestion Concours' own title. | Feuilles de juge de style |
| print_pedigree (395); prints.rs:89 « Ascendance » | mit Abstammung | avec ascendance | A horse's pedigree is « origines » (Rosson too). | avec origines / Origines |
| not_reachable (404) | …nur mit der Pilotversion erreichbar… | …qu'avec la version du pilote… | "The pilot's version". | …qu'avec la version pilote… |
| server_simulation (401), simulation_hint (406) | Simulation: nichts wird gesendet | Simulation : rien ne part | Colloquial (low). | Simulation : rien n'est envoyé |
| online_role_collaborator (468), online_accept_stay (474); notices.rs:25 | Mitarbeiter:in | Collaborateur | German is inclusive, French masculine only. This is a style decision for the whole app. | Collaborateur/trice (if the inclusive form is wanted) |
| online_err_password (493); site.rs:125 | Das Passwort hat 10 bis 200 Zeichen. | Le mot de passe a de 10 à 200 caractères. | States a fact where a rule is meant. | Le mot de passe doit compter de 10 à 200 caractères. |
| pub_title (503), not_set_up (33), as_delivered_hint (18) | Ausgeschriebene Prüfung | Épreuve publiée | Swiss usage is « avant-programme » (optional). | Épreuve de l'avant-programme |
| pub_fee (510); prints.rs `words` fee (83) « Finance », entries print « Finance 1 / 2 » (785–786) | Nenngeld | Finance | « Finance » alone is ambiguous. | Finance d'inscription (in prints: « Fin. d'inscr. 1 / 2 ») |
| pub_wrong (514) | Falschnennungen | fausses inscriptions | Sounds fake or fraudulent. | inscriptions erronées |
| every_test (525) | Alle {n} Min. (die Aufgabe) | Toutes les {n} min (le test) | « test » is an anglicism for a dressage test. | Toutes les {n} min (durée de la reprise) |
| delete_results (378), delete_results_ask (379), change_delete_results (380) vs remove_start_lists (381) | löschen | Effacer / Supprimer | Two verbs. | Supprimer |
| reference_warning (558) | …vor dem Speichern geprüft… | …vérifiée avant d'être gardée… | « gardée » (low). | …vérifiée avant l'enregistrement… |

### 5.2 `crates/office/src/prints.rs`

The judge sheet labels (lines 553–580) are marked "Rosson's words for the form" and inherit Rosson's errors.

| Line | Key | French now | Why | Proposed French |
|---|---|---|---|---|
| 554 | start_number | Numéro de start | Anglicism (see 2.2 #4). | N° de départ |
| 558 | judge | Juge en (Position) | German "/in" read as "in" (see 2.1 #28). Printed on every dressage judge sheet. | Juge (position) |
| 568 | subtotal | Total intermédiaire: | No space before the colon, and German has no colon at all. | Total intermédiaire |
| 569 | penalties | Points de pénalités | Should be singular *pénalité*. SE's own test texts say « points de pénalisation ». | Points de pénalité |
| 572 | percent | Résultat en pourcent | « pourcent » is not a noun. | Résultat en pourcentage |
| 573 | max_points | Max. de points | Unidiomatic. | Points max. |
| 578 | signature | Signature de la/du juge: | No space before the colon. | Signature du juge / de la juge : |
| 87 | passport | Passeport | It is the passport *number*. | N° de passeport |
| 89 | pedigree | Ascendance | Equestrian term is « origines ». | Origines |
| 356 | points | Pts gain | See 5.1 order_points. | per the SE term |
| 1226 | (time correction) | Correction | German says *Zeitkorrektur*. | Correction de temps |
| 1255, 1370 | pair_short | paire | « couple » (see 2.2 #1). | couple |
| 1256 | club | Club | Elsewhere « Société ». | Société |

The style-sheet criteria (1406–1450) are Swiss Equestrian's and Gestion Concours' own wording and were left as they are. One optional change on the licence sheet (1410): « cheval à la rêne » → « cheval sur la main ».

Also, in `history.rs:155`, « Rang à la main » → « Rang attribué à la main » (low).

### 5.3 `crates/server/src/site.rs` and `notices.rs`

| Location | French now | Why | Proposed French |
|---|---|---|---|
| site.rs:193, 222 | …Pour travailler sur un concours, il faut ses clés de Swiss Equestrian. / …il vous faut ses clés de Swiss Equestrian. | A show has *one* access key. The plural reads as "its keys". The German « seine Schlüssel » is ambiguous too. | …il vous faut sa clé d'accès, fournie par Swiss Equestrian. |
| site.rs:142–143 | Un problème / Le serveur n'a pas pu le faire. Réessayez plus tard. | Telegraphic, and « le faire » is vague. | Un problème est survenu / Le serveur n'a pas pu traiter la demande. Réessayez plus tard. |
| site.rs:125 | Le mot de passe a de 10 à 200 caractères. | Same as 5.1 online_err_password. | Le mot de passe doit compter de 10 à 200 caractères. |

`notices.rs`: the French e-mails are correct and natural. Nothing to change, apart from the optional inclusive « collaborateur/trice » (5.1).

### 5.4 Typography (as a class)

- `i18n.tsx` consistently uses « : », « ? », « ; » with a space and the ’ apostrophe, which is good. But the space is always a plain U+0020: none of the 52 « : » or 6 « ? » use a non-breaking space, and neither do the spaces inside « » (e.g. fast_hint « 6. »). On phone widths (U10) the colon or question mark can wrap onto its own line. Use U+202F (narrow) or U+00A0, for example through one helper applied to the French dictionary.
- `prints.rs` French strings use the straight `'` in 11 places (« L'épreuve n'a pas de juge… », « Étiquettes d'adresse », « Total de l'équipe ») while the app uses ’. Two labels have no space before « : » (568, 578).

## Driving's multitest (2026-11-14)

- « Pénalités test au sol (C) » / « Penalità test a terra (C) » for Rosson's
  « Strafpunkte Bodentest »: Rosson's French leaves « Bodentest » untranslated and neither the
  French nor the German rulebook names the ground test. To confirm.

## Driving's control sheets (2026-11-14)

- Our Italian for the sheets' own words (« Coni – foglio di controllo », « Distanza tra i coni »,
  « Supplemento », « Errori agli ostacoli », « 1° cronometro » …), to confirm; the rules'
  penalty texts print in German (no Italian edition of the driving rules).

## The desktop app's downloads and updates (2026-11-14)

- The site's download page (`crates/server/src/downloads.rs`): Windows' own SmartScreen words,
  « Windows a protégé votre ordinateur », « Informations complémentaires », « Exécuter quand
  même » and «Der Computer wurde durch Windows geschützt», «Weitere Informationen», «Trotzdem
  ausführen», as Windows 10 and 11 show them (to compare with a French and a German Windows).
  The Mac folder: « Applications » / «Programme».
- The app's updates (`update_…` in `i18n.tsx`, `i18n-it.ts`): Italian « Installa e riavvia »,
  « EquiEvents si chiude e si riapre. Tutto è salvato: riprendete da dove eravate. », to confirm.

## Swiss Equestrian's registry in the pickers (2026-11-14)

- « registre » / «Register» / «registro» as the tag of a rider or horse not yet in the show, and
  « Registre de Swiss Equestrian : … du 14.11.2026 », « pas enregistré pour 2026 » /
  «für 2026 nicht registriert» / «non registrato per il 2026» for a horse whose fee is not paid
  for the show's year (Rosson leaves those out; SE's own wording unknown), to confirm.
