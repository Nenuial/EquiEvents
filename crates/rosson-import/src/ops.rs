//! Turn a Rosson export (one event) into operations for a new show file.
//!
//! Order: what Swiss Equestrian delivered (`EventImported`), the classes as run, start lists, officials'
//! assignments, then every score card and judge sheet, and finally `ClassFinalised` for the
//! classes Rosson had marked definitive. Stamps come from a dedicated import clock, so importing
//! the same export twice gives the same operations.

use std::collections::{BTreeMap, HashMap};

use rust_decimal::Decimal;
use serde_json::Value;
use show::model::*;
use show::oplog::{Change, Clock, Operation};

use crate::{decimal, id, text, value_decimal, ImportError, NOT_STARTING};

const DEVICE: &str = "rosson-import";
/// Rosson's placeholder horse for horses without Swiss Equestrian passport.
const PLACEHOLDER_HORSE: u64 = 5;

/// Who made a show imported from a Rosson export (its operations' author): the office knows the
/// event's key came with the export.
pub const AUTHOR: &str = "Rosson import";

pub fn to_operations(snapshot: &Value) -> Result<Vec<Operation>, ImportError> {
    let store = |name: &str| snapshot.get(name).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let event_record = store("veranstaltungen").first().ok_or_else(|| ImportError("no event".into()))?;
    let nr = text(&event_record["nr"]);
    let disziplin: HashMap<String, String> = store("disziplinen").iter().map(|d| (id(&d["id"]), text(&d["code"]))).collect();
    // Roles in a class (Prüfungsfunktion) and at the event (Veranstaltungsfunktion).
    let role_code: HashMap<String, String> = store("pruefungsfunktionen").iter().map(|f| (id(&f["id"]), text(&f["code"]))).collect();
    let event_role_code: HashMap<String, String> = store("veranstaltungsfunktionen").iter().map(|f| (id(&f["id"]), text(&f["code"]))).collect();
    let category_code: HashMap<String, String> = store("pruefungskategorien").iter().map(|k| (id(&k["id"]), text(&k["code"]))).collect();
    let number = |v: &Value| v.as_u64().or_else(|| text(v).parse().ok());

    let mut clock = Clock::new(DEVICE);
    let mut ops = Vec::new();
    // Operations on each class's results, which its finalisation is based on (rule R4).
    let class_ops: std::cell::RefCell<HashMap<String, Vec<String>>> = Default::default();
    let mut push = |change: Change| {
        let stamp = clock.tick(0);
        let op_id = format!("rosson:{nr}:{}", stamp.counter);
        let class = match &change {
            Change::StartsSet { class, .. } | Change::ResultEntered { class, .. } | Change::JudgeSheetEntered { class, .. } | Change::RankSet { class, .. } => Some(class.0.clone()),
            _ => None,
        };
        if let Some(class) = class {
            class_ops.borrow_mut().entry(class).or_default().push(op_id.clone());
        }
        ops.push(Operation { id: op_id, stamp, author: AUTHOR.into(), change });
    };

    // What Swiss Equestrian delivered. People: riders as copied onto their entries, then the
    // event's officials as Rosson kept their records (`offizielle_personen`), then officials of
    // the national list who are assigned at this event.
    let mut persons: BTreeMap<u64, Person> = BTreeMap::new();
    let mut contacts: BTreeMap<u64, Contact> = BTreeMap::new();
    let mut notes = Vec::new();
    let mut horses: BTreeMap<u64, Horse> = BTreeMap::new();
    let mut entries = Vec::new();
    for n in store("nennungen") {
        let rider = number(&n["reiter_id"]).ok_or_else(|| ImportError(format!("entry {} without rider", id(&n["id"]))))?;
        persons.entry(rider).or_insert_with(|| Person {
            id: PersonId(rider),
            first_name: text(&n["reiter_vorname"]),
            last_name: text(&n["reiter_nachname"]),
            town: text(&n["reiter_ort"]),
            nationality: text(&n["reiter_land"]),
            club: text(&n["vereinsname"]),
            licences: text(&n["lizenzen"]),
            youth: text(&n["nachwuchsreiter"]),
        });
        if n.get("reiter_nachname").is_some() {
            contacts.entry(rider).or_insert_with(|| Contact {
                person: PersonId(rider),
                street: text(&n["reiter_strasse"]),
                postcode: text(&n["reiter_plz"]),
                country: text(&n["reiter_land"]),
                email: text(&n["email"]),
                phone_private: text(&n["telefon_privat"]),
                phone_mobile: text(&n["telefon_mobil"]),
                birth_date: date_of(&n["geboren_am"]),
                ..Default::default()
            });
        }
        let (remarks, companion) = (text(&n["bemerkungen"]), text(&n["begleit_person"]));
        if !remarks.is_empty() || !companion.is_empty() {
            notes.push(EntryNote { entry: EntryId(number(&n["id"]).unwrap_or_default()), remarks, companion });
        }
        // Horses of the entry (the guest horse is Rosson's placeholder, not a horse).
        for p in n["pferde"].as_array().into_iter().flatten() {
            let Some(hid) = number(&p["pferd_id"]).filter(|h| *h != PLACEHOLDER_HORSE) else { continue };
            horses.entry(hid).or_insert_with(|| Horse {
                id: HorseId(hid),
                name: text(&p["pferdename"]),
                signalement: text(&p["pferd_signalement"]),
                owner: text(&p["pferd_besitzer"]),
                breeder: text(&p["pferd_zuechter"]),
                record: horse_record(p, hid),
            });
        }
        let first_horse = n["pferde"].get(0).filter(|p| !p["pferd_id"].is_null());
        let not_starting = n["kein_start"] == true || NOT_STARTING.contains(&text(&n["status"]).as_str());
        entries.push(Entry {
            id: EntryId(number(&n["id"]).unwrap_or_default()),
            published: PublishedClassId(number(&n["ausschreibung_pruefung_id"]).unwrap_or_default()),
            rider: PersonId(rider),
            // The horse on the entry; a change of horse at the show updates it, not always `pferd_id`.
            horse: first_horse.and_then(|p| number(&p["pferd_id"])).or_else(|| number(&n["pferd_id"]).filter(|h| *h > 0)).map(HorseId),
            guest_horse: if first_horse.is_some_and(|p| number(&p["pferd_id"]) == Some(PLACEHOLDER_HORSE)) {
                first_horse.map(|p| text(&p["pferdename"]))
            } else {
                Some(text(&n["gastpferd_name"])).filter(|s| !s.is_empty())
            },
            status: if not_starting { EntryStatus::NotStarting } else { EntryStatus::Entered },
            hors_concours: n["hors_concours"] == true,
            team: Some(id(&n["equipe_id"])).filter(|s| !s.is_empty()).map(TeamId),
            programme: number(&n["programm_id"]).map(|p| p as u32),
            horses_on_entry: n["pferde"].as_array().cloned().unwrap_or_default(),
            stage: n["stufe"].as_i64(),
            rider_on_entry: n.get("reiter_nachname").map(|_| RiderOnEntry {
                first_name: text(&n["reiter_vorname"]),
                last_name: text(&n["reiter_nachname"]),
                town: text(&n["reiter_ort"]),
                club: text(&n["vereinsname"]),
            }),
            waiting_list: text(&n["status"]) == "zurueckgewiesen",
            fee_paid: decimal(&n["nenngeld_bezahlt"]),
            organiser_place: n["ist_vergebener_startplatz"] == true,
            se_status: text(&n["status"]),
            entered_by: number(&n["nenner_id"]).map(PersonId),
            other_horses: ["pferd2_id", "pferd3_id", "pferd4_id", "pferd5_id", "pferd6_id"].iter().filter_map(|f| number(&n[*f]).filter(|h| *h > 0)).map(HorseId).collect(),
            winning_points: WinningPoints { rider: decimal(&n["gwp_reiter"]), horse: decimal(&n["gwp_pferd"]), pair: decimal(&n["gwp_paar"]) },
            se_changed_at: n["geaendert_am"].as_str().map(str::to_string),
            // The office's own: not in Swiss Equestrian's data.
            rider_changed: n["reiterwechsel"] == true,
            // The office's note on the entry (Rosson's `kommentar`; a coach is often written there).
            comment: text(&n["kommentar"]),
            record: show::public::entry(n),
        });
    }
    for p in event_record["offizielle_personen"].as_array().into_iter().flatten() {
        let Some(pid) = number(&p["id"]) else { continue };
        persons.entry(pid).or_insert_with(|| Person {
            id: PersonId(pid),
            first_name: text(&p["vorname"]),
            last_name: text(&p["nachname"]),
            town: text(&p["ort"]),
            nationality: text(&p["land"]),
            club: text(&p["vereinsname"]),
            licences: text(&p["lizenzen"]),
            youth: text(&p["nachwuchsreiter"]),
        });
        contacts.entry(pid).or_insert_with(|| Contact {
            person: PersonId(pid),
            salutation: text(&p["anrede"]),
            street: text(&p["strasse"]),
            postcode: text(&p["postleitzahl"]),
            country: text(&p["land"]),
            email: text(&p["email"]),
            phone_private: text(&p["telefon_privat"]),
            phone_mobile: text(&p["telefon_mobil"]),
            language: text(&p["sprache"]),
            birth_date: date_of(&p["geboren_am"]),
        });
    }
    // The national list of officials holds everyone; only those assigned here are taken.
    let assigned: std::collections::HashSet<u64> = store("offizielle_zuweisungen")
        .iter()
        .filter_map(|z| number(&z["person_id"]))
        .chain(event_record["beteiligungen"].as_array().into_iter().flatten().filter_map(|b| number(&b["person_id"])))
        .collect();
    for o in store("offizielle").iter().filter(|o| number(&o["id"]).is_some_and(|p| assigned.contains(&p))) {
        let pid = number(&o["id"]).unwrap_or_default();
        persons.entry(pid).or_insert_with(|| Person {
            id: PersonId(pid),
            first_name: text(&o["vorname"]),
            last_name: text(&o["nachname"]),
            town: text(&o["ort"]),
            nationality: String::new(),
            club: String::new(),
            licences: String::new(),
            youth: String::new(),
        });
    }
    push(Change::EventImported {
        event: Event {
            id: EventId(number(&event_record["id"]).unwrap_or_default()),
            number: nr.clone(),
            title: ["erste_titelzeile", "zweite_titelzeile", "dritte_titelzeile"].iter().map(|k| text(&event_record[*k])).filter(|s| !s.is_empty()).collect(),
            place: text(&event_record["ort"]),
            from: text(&event_record["von"]),
            to: text(&event_record["bis"]),
            fixed_start_numbers: event_record["fixe_startnummern"] == true,
            start_numbers_assigned: event_record["startnummern_schon_vergeben"] == true,
            organiser: text(&event_record["veranstalter"]),
            postcode: text(&event_record["postleitzahl"]),
            disciplines: match &event_record["disziplin_code"] {
                Value::Array(codes) => codes.iter().map(text).collect(),
                Value::Null => Vec::new(),
                code => vec![text(code)],
            },
            se_last_change: event_record["letzte_aenderung"].as_str().map(str::to_string),
            record: show::public::event(event_record),
        },
        published_classes: store("ausschreibung_pruefungen")
            .iter()
            .map(|a| PublishedClass {
                id: PublishedClassId(number(&a["id"]).unwrap_or_default()),
                number: text(&a["nummer"]),
                discipline: disziplin.get(&id(&a["disziplin_id"])).cloned().unwrap_or_default(),
                // Rosson adds the code from its master data when it shows the class.
                category: category_code.get(&id(&a["kategorie_id"])).cloned().unwrap_or_else(|| text(&a["kategorie_code"])),
                scoring_table: number(&a["wertung_id"]).map(|w| w as u32),
                team_size: number(&a["equipe_anzahl_nennungen"]).map(|n| n as u32).filter(|n| *n > 0),
                discipline_id: number(&a["disziplin_id"]).unwrap_or_default() as u32,
                category_id: number(&a["kategorie_id"]).map(|k| k as u32),
                category_text: text(&a["kategorie_text"]),
                levels: number(&a["springen_anzahl_stufen"]).unwrap_or_default() as u32,
                date: text(&a["datum"]).get(..10).unwrap_or_default().to_string(),
                scoring_text: a["wertung_text"].as_str().map(str::to_string),
                description: text(&a["bemerkungen"]),
                driving_phases: a["fahren_zwischenpruefungen"].as_array().cloned().unwrap_or_default(),
                fee: match &a["nenngeld1"] {
                    Value::Number(n) => rust_decimal::Decimal::from_str_exact(&n.to_string()).ok(),
                    Value::String(s) => rust_decimal::Decimal::from_str_exact(s.trim()).ok(),
                    _ => None,
                },
                record: a.clone(),
            })
            .collect(),
        persons: persons.into_values().collect(),
        horses: horses.into_values().collect(),
        entries,
        teams: store("equipen")
            .iter()
            .map(|e| Team {
                id: TeamId(id(&e["id"])),
                published: PublishedClassId(number(&e["ausschreibung_pruefung_id"]).unwrap_or_default()),
                name: text(&e["name"]),
                start_number: number(&e["startnummer"]).map(|n| n as u32),
            })
            .collect(),
        scoring_tables: store("wertungen")
            .iter()
            .map(|w| ScoringTableDef { id: number(&w["id"]).unwrap_or_default() as u32, code: text(&w["code"]), name: text(&w["bezeichnung_de"]), name_fr: text(&w["bezeichnung_fr"]), name_it: text(&w["bezeichnung_it"]), definition: w["bewertung_json"].as_str().unwrap_or("").to_string() })
            .collect(),
        programmes: store("programme")
            .iter()
            .map(|p| ProgrammeDef { id: number(&p["id"]).unwrap_or_default() as u32, name: text(&p["name"]), max_points: decimal(&p["max_punkte"]).unwrap_or_default(), definition: p["programm_json"].as_str().unwrap_or("").to_string(), discipline_id: number(&p["disziplin_id"]).map(|d| d as u32) })
            .collect(),
        reasons: store("aufgabegruende")
            .iter()
            .filter_map(|g| Some(ReasonDef { id: number(&g["id"])? as u32, discipline: disziplin.get(&id(&g["disziplin_id"])).cloned(), name: text(&g["bezeichnung_de"]) }))
            .collect(),
        categories: store("pruefungskategorien")
            .iter()
            .filter_map(|k| Some(CategoryDef { id: number(&k["id"])? as u32, code: text(&k["code"]), programme: number(&k["programm_id"]).map(|p| p as u32), record: (*k).clone() }))
            .collect(),
        // Swiss Equestrian's master data as Rosson kept it (the officials without their contacts).
        master: crate::export::MASTER_STORES
            .iter()
            .map(|name| (name.to_string(), store(name).iter().map(|r| if *name == "offizielle" { show::public::person(r) } else { r.clone() }).collect()))
            .collect(),
    });
    push(Change::PrivateDataDelivered { contacts: contacts.into_values().collect(), notes });
    // Prizes, as Rosson's prize dialog keeps them on the published class.
    for a in store("ausschreibung_pruefungen") {
        let amounts: Vec<Decimal> = a["preisgelder"].as_array().into_iter().flatten().map(|v| decimal(v).unwrap_or_default().normalize()).collect();
        let natural: Vec<String> = a["naturalpreise"].as_array().into_iter().flatten().map(text).collect();
        let minimum = decimal(&a["min_preisgeld"]).map(|d| d.normalize());
        if amounts.is_empty() && natural.is_empty() && minimum.is_none() {
            continue;
        }
        let prizes = Prizes { minimum: minimum.unwrap_or_default(), amounts, natural };
        push(Change::PrizesSet { published: PublishedClassId(number(&a["id"]).unwrap_or_default()), prizes: Some(prizes) });
    }
    // Roles of officials and the national list of officials, as Rosson kept them.
    push(Change::RolesDelivered {
        roles: [("pruefungsfunktionen", true), ("veranstaltungsfunktionen", false)]
            .iter()
            .flat_map(|(name, in_class)| {
                store(name).iter().filter_map(move |r| {
                    Some(RoleDef { id: number(&r["id"])? as u32, code: text(&r["code"]), in_class: *in_class, name_de: text(&r["bezeichnung_de"]), name_fr: text(&r["bezeichnung_fr"]), name_it: text(&r["bezeichnung_it"]) })
                })
            })
            .collect(),
    });
    push(Change::OfficialsDelivered {
        officials: store("offizielle")
            .iter()
            .filter_map(|o| {
                Some(Official {
                    person: PersonId(number(&o["id"])?),
                    first_name: text(&o["vorname"]),
                    last_name: text(&o["nachname"]),
                    town: text(&o["ort"]),
                    canton: text(&o["kanton"]),
                    roles: o["rollen"].as_array().into_iter().flatten().map(|r| text(&r["rolle_bezeichnung_de"])).collect(),
                    roles_fr: o["rollen"].as_array().into_iter().flatten().map(|r| text(&r["rolle_bezeichnung_fr"])).collect(),
                    roles_it: o["rollen"].as_array().into_iter().flatten().map(|r| text(&r["rolle_bezeichnung_it"])).collect(),
                })
            })
            .collect(),
    });

    // Classes as run, start lists.
    let starter_entry: HashMap<String, (String, EntryId)> = store("starter").iter().map(|s| (id(&s["id"]), (id(&s["pruefung_id"]), EntryId(number(&s["nennung_id"]).unwrap_or_default())))).collect();
    let start_numbers: HashMap<u64, u32> = store("nennungen").iter().filter_map(|n| Some((number(&n["id"])?, number(&n["startnummer"])? as u32))).collect();
    // A published class's maximum, when entries were left out by it.
    let max_starters: HashMap<String, u32> = store("ausschreibung_pruefungen")
        .iter()
        .filter_map(|a| {
            let max = number(&a["max_startende"])? as u32;
            let entries = store("nennungen").iter().filter(|n| id(&n["ausschreibung_pruefung_id"]) == id(&a["id"])).count();
            (max > 0 && (max as usize) < entries).then(|| (id(&a["id"]), max))
        })
        .collect();
    for p in store("pruefungen") {
        let class_id = ClassId(id(&p["id"]));
        let not_covered = ["idealzeit_berechnen", "max_zeit_berechnen", "kombination", "fahren"].iter().filter(|f| p[**f] == true).map(|f| f.to_string()).collect();
        let mut class = Class {
                id: class_id.clone(),
                published: PublishedClassId(number(&p["ausschreibung_pruefung_id"]).unwrap_or_default()),
                number: text(&p["nummer"]),
                name: text(&p["name"]),
                date: crate::swiss_time::date(&text(&p["datum"])).unwrap_or_else(|| text(&p["datum"])),
                scoring_table: number(&p["wertung_id"]).map(|w| w as u32),
                programme: number(&p["programm_id"]).map(|p| p as u32),
                options: ClassOptions {
                    medal_jump_offs: false,
                    placed_count: if p["customKlassierte"] == true { number(&p["anzahlKlassiert"]).map(|n| n as u32) } else { None },
                    totals_per_judge: p["manuelle_eingabe"] == true,
                    team_3_2: p["mannschaft"] == true,
                    not_covered,
                    team_only: false,
                    // Rosson always dropped the worst of a team of four (S7).
                    team_count_all: false,
                    // Rosson has no Tag der Jugend final tie-break (S38).
                    youth_final: false,
                    // Rosson has no qualifier (U9).
                    qualifier: None,
                    // Rosson writes its maximum even when it cut nobody: kept when it did.
                    max_starters: max_starters.get(&id(&p["ausschreibung_pruefung_id"])).copied(),
                    ideal_time: ideal_time(&p["idealzeit"]),
                    rank_points: p["wertung_nach"] == "rangpunkte",
                    ideal_time_penalties: false,
                    max_time: None,
                    max_time_penalties: false,
                    driving: None,
                },
                status: ClassStatus::Running,
                extra_fields: p["weitere_felder"].as_array().map(|fields| {
                    fields.iter().map(|f| ExtraField { key: text(&f["key"]), value: text(&f["value"]) }).collect()
                }),
                position: number(&p["position"]).map(|n| n as u32),
                time_text: text(&p["uhrzeit"]),
                scoring_text: p["wertung_text"].as_str().map(str::to_string),
                logo: p.get("logo").cloned(),
                start_numbers_assigned: p["startnummern_schon_vergeben"] == true,
                teams: {
                    let mut teams: Vec<&Value> = p["equipen"].as_array().map(|t| t.iter().collect()).unwrap_or_default();
                    teams.sort_by_key(|t| number(&t["reihenfolge"]).unwrap_or(u64::MAX));
                    teams.iter().map(|t| TeamId(id(&t["id"]))).collect()
                },
                start_list: start_list_setup(p, &text),
                sponsor: text(&p["sponsor_etiketten"]),
                phase: None,
                phases: Vec::new(),
            };
        // Eventing: the class made of phases; its phases (Rosson's Zwischenprüfungen) are classes.
        let mut phase_classes = Vec::new();
        if p["kombination"] == true || p["fahren"] == true {
            let mut records: Vec<&Value> = store("zwischenpruefungen").iter().filter(|z| id(&z["parent_pruefung_id"]) == class_id.0).collect();
            records.sort_by_key(|z| number(&z["sorter"]).unwrap_or(u64::MAX));
            for z in records {
                let Some(kind) = PhaseKind::from_name(&text(&z["code"])) else { continue };
                let phase = Class {
                    id: ClassId(id(&z["id"])),
                    date: crate::swiss_time::date(&text(&z["datum"])).unwrap_or_else(|| class.date.clone()),
                    scoring_table: number(&z["wertung_id"]).map(|w| w as u32),
                    programme: number(&z["programm_id"]).map(|p| p as u32),
                    options: ClassOptions {
                        driving: driving_settings(kind, z),
                        totals_per_judge: z["manuelle_eingabe"] == true,
                        placed_count: if z["customKlassierte"] == true { number(&z["anzahlKlassiert"]).map(|n| n as u32) } else { None },
                        ideal_time: ideal_time(&z["idealzeit"]),
                        ideal_time_penalties: z["idealzeit_berechnen"] == true,
                        max_time: max_time(&z["max_zeit"]),
                        max_time_penalties: z["max_zeit_berechnen"] == true,
                        ..ClassOptions::default()
                    },
                    position: number(&z["position"]).map(|n| n as u32),
                    time_text: text(&z["uhrzeit"]),
                    scoring_text: z["wertung_text"].as_str().map(str::to_string),
                    start_list: start_list_setup(z, &text),
                    teams: Vec::new(),
                    phase: Some(Phase { of: class_id.clone(), kind }),
                    phases: Vec::new(),
                    ..class.clone()
                };
                class.phases.push(phase.id.clone());
                phase_classes.push(phase);
            }
            // The time settings are the phases'; the class is not "not covered" any more.
            class.options.not_covered.retain(|f| !matches!(f.as_str(), "kombination" | "fahren" | "idealzeit_berechnen" | "max_zeit_berechnen"));
            class.programme = None;
            class.scoring_table = None;
        }
        push(Change::class_set(class));
        for phase in phase_classes {
            let phase_id = phase.id.clone();
            push(Change::class_set(phase));
            let mut starts: Vec<Start> = store("starter")
                .iter()
                .filter(|s| id(&s["zwischenpruefung_id"]) == phase_id.0)
                .map(|s| {
                    let entry = number(&s["nennung_id"]).unwrap_or_default();
                    Start { entry: EntryId(entry), order: number(&s["reihenfolge"]).unwrap_or_default() as u32, start_number: start_numbers.get(&entry).copied(), team_order: None }
                })
                .collect();
            starts.sort_by_key(|s| s.order);
            push(Change::starts_set(phase_id, starts));
        }
        let mut starts: Vec<Start> = store("starter")
            .iter()
            .filter(|s| id(&s["pruefung_id"]) == class_id.0 && id(&s["zwischenpruefung_id"]).is_empty())
            .map(|s| {
                let entry = number(&s["nennung_id"]).unwrap_or_default();
                Start {
                    entry: EntryId(entry),
                    order: number(&s["reihenfolge"]).unwrap_or_default() as u32,
                    start_number: start_numbers.get(&entry).copied(),
                    team_order: number(&s["equipe_reihenfolge"]).map(|n| n as u32),
                }
            })
            .collect();
        starts.sort_by_key(|s| s.order);
        push(Change::starts_set(class_id, starts));
    }

    // Officials. Eventing: the judges of a class made of phases judge its dressage phase.
    let dressage_phase: HashMap<String, String> = store("zwischenpruefungen")
        .iter()
        .filter(|z| PhaseKind::from_name(&text(&z["code"])) == Some(PhaseKind::Dressage))
        .map(|z| (id(&z["parent_pruefung_id"]), id(&z["id"])))
        .collect();
    for z in store("offizielle_zuweisungen") {
        push(Change::AssignmentSet {
            assignment: Assignment {
                id: AssignmentId(id(&z["id"])),
                class: Some(id(&z["pruefung_id"])).filter(|s| !s.is_empty()).map(|c| if role_code.get(&id(&z["rolle_id"])).is_some_and(|r| r == "DRI") { dressage_phase.get(&c).cloned().unwrap_or(c) } else { c }).map(ClassId),
                person: PersonId(number(&z["person_id"]).unwrap_or_default()),
                role: if z["pruefung_id"].is_null() || id(&z["pruefung_id"]).is_empty() { &event_role_code } else { &role_code }.get(&id(&z["rolle_id"])).cloned().unwrap_or_default(),
                letter: Some(text(&z["buchstabe"])).filter(|s| !s.is_empty()),
                role_id: number(&z["rolle_id"]).map(|r| r as u32),
                event: number(&z["veranstaltung_id"]).map(EventId),
                se_id: number(&z["equis_id"]),
                on_event: z["auf_veranstaltung"].as_bool(),
                responsible: z["verantwortlicher"].as_bool(),
                readonly: z["readonly"].as_bool(),
                days: number(&z["anzahl_einsatztage"]).map(|d| d as u32),
                updated_at: number(&z["updated_at"]),
                looked_up: false,
            },
        });
    }

    // Driving phases' results are read field by field (their details are Rosson's own shapes).
    let phase_kinds: HashMap<String, PhaseKind> = store("zwischenpruefungen").iter().filter_map(|z| PhaseKind::from_name(&text(&z["code"])).map(|k| (id(&z["id"]), k))).collect();
    // Score cards and judge sheets: only what was entered (computed fields are not imported).
    let assignment_ids: std::collections::HashSet<String> = store("offizielle_zuweisungen").iter().map(|z| id(&z["id"])).collect();
    for r in store("resultate") {
        // The overall rows of eventing are computed (typ "gesamt").
        if r["typ"] == "gesamt" || r["gestrichen"] == true {
            continue;
        }
        // Eventing: a phase's result is the phase's.
        let class = ClassId(Some(id(&r["zwischenpruefung_id"])).filter(|z| !z.is_empty()).unwrap_or_else(|| id(&r["pruefung_id"])));
        // A rank set by hand (Rosson: customRang), on a rider's or a team's result (S18).
        let manual = number(&r["customRang"]).filter(|r| *r > 0).map(|r| r as u32);
        let Some((_, entry)) = starter_entry.get(&id(&r["starter_id"])) else {
            // Team rows are computed, except a rank set by hand, and team dressage's sheets.
            let team = Some(id(&r["equipe_id"])).filter(|t| !t.is_empty()).map(TeamId);
            if let (Some(rank), Some(team)) = (manual, &team) {
                push(Change::RankSet { class: class.clone(), competitor: Competitor::Team(team.clone()), rank: Some(rank), reason: "set by hand in Rosson".into() });
            }
            if let (Some(team), "dressur") = (team, r["typ"].as_str().unwrap_or("")) {
                let details = r["details"].as_object();
                if let Some(code) = details.and_then(|d| d.get("aufgabe")).and_then(decimal).filter(|a| *a > Decimal::ZERO) {
                    push(Change::TeamResultEntered { class: class.clone(), team: team.clone(), values: BTreeMap::new(), out: Some(code.try_into().unwrap_or(0)) });
                }
                for (judge, marks_json) in details.into_iter().flatten().filter(|(k, _)| assignment_ids.contains(*k)) {
                    let Some(sheet) = judge_sheet(r, judge, marks_json) else { continue };
                    let marks = sheet.marks.into_iter().map(|(m, v)| (m, Some(v))).collect();
                    let penalties = Some(SheetPenalties { points: sheet.penalty_points, percent: sheet.penalty_percent, excluded: sheet.excluded });
                    push(Change::TeamSheetEdited { class: class.clone(), team: team.clone(), judge: AssignmentId(judge.clone()), marks, penalties });
                }
            }
            continue;
        };
        if let Some(rank) = manual {
            push(Change::RankSet { class: class.clone(), competitor: Competitor::Entry(entry.clone()), rank: Some(rank), reason: "set by hand in Rosson".into() });
        }
        let details = r["details"].as_object();
        let out = details.and_then(|d| d.get("aufgabe")).and_then(decimal).filter(|a| *a > Decimal::ZERO).map(|a| a.try_into().unwrap_or(0));
        let mut values = BTreeMap::new();
        if let Some(kind) = phase_kinds.get(&class.0).filter(|k| matches!(k, PhaseKind::Marathon | PhaseKind::Cones | PhaseKind::Derby | PhaseKind::Open)) {
            values = driving_values(*kind, &r["details"]);
        }
        for (field, v) in details.into_iter().flatten().filter(|_| !phase_kinds.get(&class.0).is_some_and(|k| matches!(k, PhaseKind::Marathon | PhaseKind::Cones | PhaseKind::Derby | PhaseKind::Open))) {
            if field.starts_with("zeit_") || field.starts_with("strafpunkte_") {
                // Cross-country times: "mm:ss".
                let value = match v.as_str().filter(|t| field.starts_with("zeit_") && t.contains(':')) {
                    Some(_) => ideal_time(v),
                    None => value_decimal(v).map_err(|e| ImportError(format!("result {}: {field}: {e}", id(&r["id"]))))?,
                };
                if let Some(d) = value {
                    values.insert(field.clone(), d);
                }
            }
        }
        push(Change::ResultEntered { class: class.clone(), entry: entry.clone(), values, out });

        for (judge, marks_json) in details.into_iter().flatten().filter(|(k, _)| assignment_ids.contains(*k)) {
            let Some(sheet) = judge_sheet(r, judge, marks_json) else { continue };
            push(Change::JudgeSheetEntered { class: class.clone(), entry: entry.clone(), judge: AssignmentId(judge.clone()), sheet });
        }
    }

    // Rankings over several classes (Rosson's finals): ranks added, or the mean percentage of
    // dressage classes; ties first by the first dressage class (Rosson), else the first class.
    let known: std::collections::HashSet<String> = store("pruefungen").iter().map(|p| id(&p["id"])).collect();
    for f in store("finalpruefungen") {
        let mut parts: Vec<&Value> = f["teilpruefungen"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
        parts.sort_by_key(|t| number(&t["sorter"]).unwrap_or(0));
        let parts: Vec<&Value> = parts.into_iter().filter(|t| known.contains(&id(&t["id"]))).collect();
        let Some(first) = parts.iter().find(|t| t["istDressur"] == true).or(parts.first()) else { continue };
        let method = if f["wertung_nach"] == "prozentpunkte" && parts.iter().all(|t| t["istDressur"] == true) {
            CombinedMethod::Percentages { tie_break: None }
        } else {
            CombinedMethod::Ranks { tie_break: ClassId(id(&first["id"])) }
        };
        let extra_fields = f["weitereFelder"].as_array().map(|fields| fields.iter().map(|x| ExtraField { key: text(&x["key"]), value: text(&x["value"]) }).filter(|x| !x.key.is_empty() || !x.value.is_empty()).collect()).unwrap_or_default();
        push(Change::CombinedSet {
            ranking: CombinedRanking {
                id: id(&f["id"]),
                name: text(&f["titel"]),
                classes: parts.iter().map(|t| ClassId(id(&t["id"]))).collect(),
                method,
                number: Some(text(&f["nummer"])).filter(|n| !n.is_empty()),
                extra_fields,
                record: f.clone(),
            },
        });
    }

    for p in store("pruefungen").iter().filter(|p| p["definitiv"] == true) {
        let based_on = class_ops.borrow().get(&id(&p["id"])).cloned().unwrap_or_default();
        push(Change::ClassFinalised { class: ClassId(id(&p["id"])), based_on });
    }
    Ok(ops)
}

/// A horse's record as far as Rosson copied it onto an entry (`pferdeAnreichern`, reversed):
/// exports hold no horse records.
fn horse_record(copy: &Value, id: u64) -> Value {
    let mut record = serde_json::Map::new();
    record.insert("id".into(), id.into());
    for (to, from) in [
        ("name", "pferdename"),
        ("signalement_label", "pferd_signalement"),
        ("besitzer", "pferd_besitzer"),
        ("mitbesitzer", "pferd_mitbesitzer"),
        ("zuechter", "pferd_zuechter"),
        ("hat_pony_startbewilligung", "hat_pony_startbewilligung"),
        ("pony_groessenkategorie", "pony_groessenkategorie"),
        ("vater_name", "vater_name"),
        ("mutter_name", "mutter_name"),
        ("vater_der_mutter_name", "vater_der_mutter_name"),
    ] {
        if let Some(v) = copy.get(from) {
            record.insert(to.into(), v.clone());
        }
    }
    Value::Object(record)
}

/// A birth date as `YYYY-MM-DD` (Rosson keeps what Swiss Equestrian sent, or a Date).
fn date_of(v: &Value) -> Option<String> {
    v.as_str().filter(|s| s.len() >= 10).map(|s| s[..10].to_string())
}

/// A class's start list setup from Rosson's fields (`type`, `proBlock`, `bloecke`, `zeit`,
/// `programmDuration`, `pausen`, `nachruecken`, `platzpflege_nach`, `platzpflege_text`).
fn start_list_setup(p: &Value, text: &dyn Fn(&Value) -> String) -> StartListSetup {
    let move_up = p["nachruecken"] == true;
    let time = |v: &Value| v.as_str().and_then(crate::swiss_time::time_of_day);
    let layout = match p["type"].as_str() {
        Some("block") => StartListLayout::Blocks {
            size: p["proBlock"].as_u64().unwrap_or(0) as u32,
            headings: p["bloecke"].as_array().map(|b| b.iter().map(text).collect()).unwrap_or_default(),
            move_up,
        },
        Some("time") => match (time(&p["zeit"]), p["programmDuration"].as_u64()) {
            (Some(first), Some(minutes)) => StartListLayout::StartTimes {
                first,
                minutes: minutes as u32,
                breaks: p["pausen"].as_array().into_iter().flatten().filter_map(|b| Some((time(&b["start"])?, time(&b["end"])?))).collect(),
                move_up,
            },
            _ => StartListLayout::Plain,
        },
        _ => StartListLayout::Plain,
    };
    let after = p["platzpflege_nach"].as_u64().unwrap_or(0) as u32;
    StartListSetup { layout, line_after: (after > 0).then(|| (after, text(&p["platzpflege_text"]))) }
}

/// Rosson's ideal time ("mm:ss", or seconds), in seconds.
/// A time Rosson writes "h:m:s" (marathon settings), in seconds.
fn hms(v: &Value) -> Option<rust_decimal::Decimal> {
    let parts: Vec<rust_decimal::Decimal> = v.as_str()?.trim().split(':').map(|p| p.trim().parse().ok()).collect::<Option<Vec<_>>>()?;
    let seconds = match parts.as_slice() {
        [h, m, s] => h * rust_decimal::Decimal::from(3600) + m * rust_decimal::Decimal::from(60) + s,
        [m, s] => m * rust_decimal::Decimal::from(60) + s,
        _ => return None,
    };
    (seconds > rust_decimal::Decimal::ZERO).then(|| seconds.normalize())
}

/// A driving phase's settings from Rosson's phase record (`setDefaults`, the settings screens).
fn driving_settings(kind: PhaseKind, z: &Value) -> Option<DrivingSettings> {
    let dec = |v: &Value| crate::decimal(v);
    Some(match kind {
        PhaseKind::Marathon => DrivingSettings {
            a_allowed: hms(&z["phase_a"]["erlaubte_zeit"]),
            a_minimum: hms(&z["phase_a"]["min_zeit"]),
            b_allowed: hms(&z["phase_b"]["erlaubte_zeit"]),
            b_minimum: hms(&z["phase_b"]["min_zeit"]),
            a_maximum: hms(&z["phase_a"]["max_zeit"]),
            b_maximum: hms(&z["phase_b"]["max_zeit"]),
            transfer: dec(&z["phase_b"]["ueberfuehrungszeit"]),
            factor: dec(&z["phase_b"]["faktor"]).or(Some(rust_decimal::Decimal::new(25, 2))),
            obstacles: crate::decimal(&z["phase_b"]["anzahl_hindernisse"]).and_then(|n| n.to_string().parse().ok()).or(Some(5)),
            ..Default::default()
        },
        PhaseKind::Cones => {
            // Rosson's driving tables: A 1, A2U 2, B 5, B2U 6, JC2U 7.
            let (cones, rounds) = match table_id(z).unwrap_or(1) {
                2 => (ConesKind::A, 2),
                5 => (ConesKind::B, 1),
                6 => (ConesKind::B, 2),
                7 => (ConesKind::JuniorCup, 2),
                _ => (ConesKind::A, 1),
            };
            let (mut allowed, mut maximum) = (BTreeMap::new(), BTreeMap::new());
            for r in 1..=2 {
                for (k, v) in z[format!("umgang{r}")].as_object().into_iter().flatten() {
                    if let (Some(licence), Some(seconds)) = (k.strip_prefix("erlaubte_zeit_"), dec(v)) {
                        allowed.insert(format!("{r}:{licence}"), seconds);
                    }
                    if let (Some(licence), Some(seconds)) = (k.strip_prefix("max_zeit_"), dec(v)) {
                        maximum.insert(format!("{r}:{licence}"), seconds);
                    }
                }
            }
            let bonus = z["bonus"].as_object().into_iter().flatten().filter_map(|(g, s)| dec(s).map(|s| (g.clone(), s))).collect();
            DrivingSettings { cones: Some(cones), rounds: Some(rounds), allowed, maximum, international: z["mitInternationalerWertung"] == true, bonus, ..Default::default() }
        }
        PhaseKind::Derby => DrivingSettings { time_factor: dec(&z["zeitfaktor"]).or(Some(rust_decimal::Decimal::new(25, 2))), ..Default::default() },
        PhaseKind::Open => DrivingSettings {
            open: Some(match table_id(z).unwrap_or(1) {
                2 => OpenKind::Points,
                3 => OpenKind::PenaltyPoints,
                4 => OpenKind::PointsThenTime,
                _ => OpenKind::Time,
            }),
            ..Default::default()
        },
        PhaseKind::Multitest => DrivingSettings { multitest: Some(text_of(&z["multitest_typ"]).unwrap_or_else(|| "B4".into())), ..Default::default() },
        _ => return None,
    })
}

/// A phase's table id (Rosson's own driving tables), a number or text.
fn table_id(z: &Value) -> Option<u64> {
    z["wertung_id"].as_u64().or_else(|| z["wertung_id"].as_str().and_then(|s| s.trim().parse().ok()))
}

fn text_of(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string).filter(|s| !s.is_empty())
}

/// A driving phase's result, field by field, from Rosson's details (marathon: phases A and B with
/// start and finish date-times, obstacles 0-based in phase B; cones: per round; derby; open).
fn driving_values(kind: PhaseKind, details: &Value) -> BTreeMap<String, rust_decimal::Decimal> {
    let mut values = BTreeMap::new();
    let mut put = |k: String, v: Option<rust_decimal::Decimal>| {
        if let Some(v) = v {
            values.insert(k, v);
        }
    };
    let dec = |v: &Value| crate::decimal(v);
    match kind {
        PhaseKind::Marathon => {
            for p in ["a", "b"] {
                let phase = &details[format!("phase_{p}")];
                put(format!("{p}_start"), phase["startzeit"].as_str().and_then(crate::swiss_time::seconds_of_day));
                put(format!("{p}_finish"), phase["zielzeit"].as_str().and_then(crate::swiss_time::seconds_of_day));
                put(format!("{p}_penalties"), dec(&phase["punkte"]));
                put(format!("{p}_neutralised"), dec(&phase["neutralisation"]));
            }
            for i in 0..12 {
                let o = &details["phase_b"][format!("hindernis_{i}")];
                put(format!("h{}_time", i + 1), dec(&o["zeit"]));
                put(format!("h{}_penalties", i + 1), dec(&o["punkte"]));
                put(format!("h{}_neutralised", i + 1), dec(&o["neutralisation"]));
            }
        }
        PhaseKind::Cones => {
            for r in 1..=2 {
                let round_ = &details[format!("umgang{r}")];
                put(format!("zeit_{r}"), dec(&round_["zeit"]));
                put(format!("hindernis_{r}"), dec(&round_["hindernis_fehler"]));
                put(format!("andere_{r}"), dec(&round_["andere_fehler"]));
            }
        }
        PhaseKind::Derby => {
            put("zeit".into(), dec(&details["gefahrene_zeit"]));
            put("abwuerfe".into(), dec(&details["strafsekunden_abwuerfe"]));
            put("uebrige".into(), dec(&details["strafsekunden_uebrige"]));
        }
        PhaseKind::Open => {
            put("sekunden".into(), dec(&details["sekunden"]));
            put("punkte".into(), dec(&details["punkte"]));
        }
        _ => {}
    }
    values
}

/// Rosson's maximum time, "mm:ss:hh" (minutes, seconds, hundredths), in seconds.
fn max_time(v: &Value) -> Option<rust_decimal::Decimal> {
    let parts: Vec<u32> = v.as_str()?.trim().split(':').map(|p| p.trim().parse().ok()).collect::<Option<Vec<_>>>()?;
    let seconds = match parts.as_slice() {
        [m, s, h] => rust_decimal::Decimal::from(m * 60 + s) + rust_decimal::Decimal::new(*h as i64, 2),
        [m, s] => rust_decimal::Decimal::from(m * 60 + s),
        _ => return None,
    };
    (seconds > rust_decimal::Decimal::ZERO).then(|| seconds.normalize())
}

fn ideal_time(v: &Value) -> Option<rust_decimal::Decimal> {
    let seconds = match v {
        Value::String(t) => match t.trim().split_once(':') {
            Some((m, s)) => rust_decimal::Decimal::from(m.trim().parse::<u32>().ok()? * 60) + s.trim().parse::<rust_decimal::Decimal>().ok()?,
            None => t.trim().parse().ok()?,
        },
        Value::Number(n) => n.to_string().parse().ok()?,
        _ => return None,
    };
    (seconds > rust_decimal::Decimal::ZERO).then(|| seconds.normalize())
}

/// A judge's sheet in a Rosson result: the marks (or the judge's total, a number, with
/// `manuelle_eingabe`) and the ticked penalties. `None`: nothing for that judge.
fn judge_sheet(r: &Value, judge: &str, marks_json: &Value) -> Option<JudgeSheet> {
    let total = if marks_json.is_number() || marks_json.is_string() { value_decimal(marks_json).ok().flatten() } else { None };
    let marks = match marks_json.as_object() {
        Some(m) => m.iter().filter(|(m, _)| !m.starts_with("_origin_")).filter_map(|(m, v)| Some((m.clone(), value_decimal(v).ok().flatten()?))).collect(),
        None if total.is_some() => BTreeMap::new(),
        None => return None,
    };
    let mut sheet = JudgeSheet { marks, total, ..Default::default() };
    for list in r["strafpunkte"].get(judge).and_then(Value::as_object).into_iter().flat_map(|t| t.values()).filter_map(Value::as_array) {
        for p in list.iter().filter(|p| p["checked"] == true) {
            if let Some(points) = decimal(&p["strafpunkte"]).filter(|d| !d.is_zero()) {
                sheet.penalty_points.push(points);
            } else if let Some(percent) = decimal(&p["strafprozente"]).filter(|d| !d.is_zero()) {
                sheet.penalty_percent.push(percent);
            } else {
                // The exclusion box (no value): Rosson ranks the rider anyway (S40).
                sheet.excluded = true;
            }
        }
    }
    Some(sheet)
}
