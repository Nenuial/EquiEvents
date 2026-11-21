//! The show office application, for every front end: the desktop app (Tauri) and, during
//! development, a browser through `office-dev-server`.
//!
//! - The **library**: the shows on this computer, one show file each (`<library>/<id>.show`).
//! - **Imports**: a Rosson export (one event, or a whole database: pick the event), as the event
//!   ended or as Swiss Equestrian delivered it (to rehearse); an EquiEvents show file.
//! - **Exports**: one show to a `.equievents` file, with or without private data.
//! - **Screens**: what each one shows ([`views`]).
//! - **equievents.ch**: the account, shows on the server, the sync ([`sync`]).
//!
//! One entry point, [`Office::call`]: a command name and JSON arguments, a JSON answer. The front
//! ends only pass them through, so they cannot drift apart.

pub mod conflicts;
pub mod connection;
pub mod entries;
pub mod exports;
pub mod history;
pub mod italian;
pub mod prepare;
pub mod prints;
pub mod registry;
pub mod results;
pub mod sections;
pub mod sync;
pub mod training;
pub mod views;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use show::model::{ClassId, EntryId, HorseId, PublishedClassId};
use show::oplog::{Clock, Operation};
use show::Store;

/// Show files in the library.
pub const SHOW_EXTENSION: &str = "show";
/// Author of the operations of a show imported as Swiss Equestrian delivered it (a rehearsal).
const REHEARSAL: &str = "import (as delivered, to rehearse)";

/// What can be undone in a show, last on top: what was done (a command name), and the changes
/// that undo it.
type UndoStack = Vec<(String, Vec<show::oplog::Change>)>;

/// Put changes on top of a show's undo (or redo) stack, which keeps the last hundred.
fn push(stacks: &Mutex<HashMap<String, UndoStack>>, id: &str, what: String, changes: Vec<show::oplog::Change>) {
    let mut stacks = stacks.lock().unwrap_or_else(|e| e.into_inner());
    let stack = stacks.entry(id.to_string()).or_default();
    stack.push((what, changes));
    if stack.len() > 100 {
        stack.remove(0);
    }
}

pub struct Office {
    library: PathBuf,
    device: String,
    open: Mutex<HashMap<String, Store>>,
    random: Mutex<u64>,
    /// This device's clock: every change gets a stamp no other change of this device has.
    clock: Mutex<Clock>,
    /// Who makes the changes (the operation log's author).
    operator: String,
    /// Per show, the changes that undo the office's last ones, with what they were.
    undo: Mutex<HashMap<String, UndoStack>>,
    /// Per show, the changes that redo what was just undone (emptied by any new change).
    redo: Mutex<HashMap<String, UndoStack>>,
    /// Access keys (the system keychain in the desktop app).
    secrets: Box<dyn connection::Secrets>,
    /// The simulated Swiss Equestrian.
    simulation: connection::Simulation,
    /// Swiss Equestrian's riders and horses, for every show.
    registry: registry::Registry,
    /// Tests: the riders and horses from the simulation (the library's), never Swiss Equestrian.
    simulated_registry: std::sync::atomic::AtomicBool,
    /// The sync with equievents.ch while the app runs.
    sync: sync::Syncing,
    /// The last commands that failed (also in `<library>/errors.log`).
    errors: Mutex<std::collections::VecDeque<Value>>,
}

/// Settings of the library on this computer (not of the shows): `<library>/settings.json`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct Settings {
    /// The highest Rosson version seen in its update feed (`se_api::version`).
    #[serde(default)]
    rosson_version: Option<String>,
    /// equievents.ch, or another server (development).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    server_url: Option<String>,
    /// Who is signed in there (the token is kept as a secret).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    account: Option<sync::Account>,
    /// Sheets of sticky labels the office described (Rosson's custom templates).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    label_sheets: Vec<prints::Sheet>,
}

fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn arg<'a>(args: &'a Value, name: &str) -> Result<&'a str, String> {
    args[name].as_str().ok_or_else(|| format!("missing argument {name}"))
}

/// The entries a change is for: `entries` (several, e.g. a pair's in the classes picked) or
/// `entry` (one).
fn entries_arg(args: &Value) -> Result<Vec<EntryId>, String> {
    match args["entries"].as_array() {
        Some(list) => list.iter().map(|e| e.as_u64().map(EntryId).ok_or_else(|| "entries: numbers".to_string())).collect(),
        None => Ok(vec![EntryId(args["entry"].as_u64().ok_or("missing argument entry")?)]),
    }
}

impl Office {
    /// The office on a library folder (created if needed). `device`: this installation's name in
    /// the operation log.
    /// Access keys are kept in `<library>/secrets.json`, readable by its owner only
    /// (development, tests); see [`Office::with_secrets`].
    pub fn new(library: &Path, device: &str) -> Result<Office, String> {
        Office::with_secrets(library, device, Box::new(connection::FileSecrets::new(&library.join("secrets.json"))))
    }

    /// The office with its own store of access keys (the desktop app: the system keychain).
    pub fn with_secrets(library: &Path, device: &str, secrets: Box<dyn connection::Secrets>) -> Result<Office, String> {
        std::fs::create_dir_all(library).map_err(|e| format!("{}: {e}", library.display()))?;
        let operator = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "office".into());
        Ok(Office {
            library: library.to_path_buf(),
            device: device.into(),
            open: Mutex::new(HashMap::new()),
            random: Mutex::new(now_millis() | 1),
            clock: Mutex::new(Clock::new(device)),
            operator,
            undo: Mutex::new(HashMap::new()),
            redo: Mutex::new(HashMap::new()),
            secrets,
            simulation: connection::Simulation::default(),
            registry: registry::Registry::new(library),
            simulated_registry: std::sync::atomic::AtomicBool::new(false),
            sync: sync::Syncing::default(),
            errors: Mutex::default(),
        })
    }

    /// Run a command. Errors are messages for the office; each is kept in the error log (the
    /// last 200 here, every one in `<library>/errors.log`, for support).
    pub fn call(&self, command: &str, args: &Value) -> Result<Value, String> {
        let answer = self.run(command, args);
        if let Err(e) = &answer {
            self.log_error(command, args, e);
        }
        answer
    }

    fn log_error(&self, command: &str, args: &Value, error: &str) {
        // What was asked, without passwords or keys.
        let mut asked = args.clone();
        if let Some(map) = asked.as_object_mut() {
            for secret in ["password", "access_key", "token"] {
                if map.contains_key(secret) {
                    map.insert(secret.into(), json!("•••"));
                }
            }
        }
        let entry = json!({ "millis": now_millis(), "command": command, "show": args["show"], "error": error, "args": asked });
        if let Ok(mut errors) = self.errors.lock() {
            errors.push_back(entry.clone());
            while errors.len() > 200 {
                errors.pop_front();
            }
        }
        let path = self.library.join("errors.log");
        // Started again from empty past a megabyte.
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > 1_000_000) {
            let _ = std::fs::remove_file(&path);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = std::io::Write::write_all(&mut file, format!("{entry}\n").as_bytes());
        }
    }

    fn run(&self, command: &str, args: &Value) -> Result<Value, String> {
        match command {
            "library" => Ok(Value::Array(self.library()?)),
            "rosson_events" => {
                let export = read_rosson(Path::new(arg(args, "path")?))?;
                Ok(Value::Array(export.events().into_iter().map(|e| json!({ "id": e.id, "number": e.number, "title": e.title, "from": e.from, "to": e.to, "entries": e.entries, "results": e.results })).collect()))
            }
            "import_rosson" => {
                let delivered = match arg(args, "mode")? {
                    "ended" => false,
                    "delivered" => true,
                    other => return Err(format!("unknown import mode {other}")),
                };
                let id = self.import_rosson(Path::new(arg(args, "path")?), arg(args, "event")?, delivered)?;
                Ok(json!({ "show": id }))
            }
            "import_show" => Ok(json!({ "show": self.import_show(Path::new(arg(args, "path")?))? })),
            // Training shows (the settings): the practice event, Swiss Equestrian's demo event.
            "create_practice" => Ok(json!({ "show": self.create_practice(args["language"].as_str().unwrap_or("fr"))? })),
            "load_demo" => Ok(json!({ "show": self.load_demo(args["driving"] == true)? })),
            "export_show" => {
                let privacy = if args["private"] == true { show::export::Privacy::All } else { show::export::Privacy::Public };
                self.export_show(arg(args, "show")?, Path::new(arg(args, "path")?), privacy)?;
                Ok(Value::Null)
            }
            // The show in Rosson's format (one event, as Rosson exports it): a copy that also opens
            // in Rosson 3.3.15. Contacts and remarks only with `private`.
            "export_rosson" => {
                let path = Path::new(arg(args, "path")?);
                let private = args["private"] == true;
                let show = arg(args, "show")?;
                // With the event's access key, as Rosson's own export (Rosson needs it).
                let key = self.event_id(show).ok().map(|event| self.secrets.get(&connection::access_key_name(event))).transpose()?.flatten();
                let bytes = self.with(show, |_, store| Ok(rosson_import::to_rosson::file(&rosson_import::to_rosson::event_stores(store.state(), private, key.as_deref())?)))?;
                std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
                Ok(Value::Null)
            }
            "rosson_file_name" => self.with(arg(args, "show")?, |_, store| Ok(json!(rosson_import::to_rosson::file_name(store.state(), &rfc3339(now_millis())[..10])))),
            "delete_show" => {
                self.delete_show(arg(args, "show")?)?;
                Ok(Value::Null)
            }
            "show" => self.with(arg(args, "show")?, |id, store| Ok(json_of(&views::show(id, store.state())))),
            "participants" => self.with(arg(args, "show")?, |_, store| Ok(json_of(&views::participants(store.state())))),
            "start_list" => {
                let class = ClassId(arg(args, "class")?.into());
                self.with(arg(args, "show")?, |_, store| Ok(json_of(&views::start_list(store.state(), &class)?)))
            }
            "ranking" => {
                let class = ClassId(arg(args, "class")?.into());
                self.with(arg(args, "show")?, |_, store| Ok(json_of(&views::ranking(store.state(), &class)?)))
            }
            // Eventing: the overall ranking with each phase's result.
            "overall" => {
                let class = ClassId(arg(args, "class")?.into());
                self.with(arg(args, "show")?, |_, store| Ok(json_of(&views::overall(store.state(), &class)?)))
            }
            "add_presentation" => {
                // Driving: a presentation judged standing, as a phase of its own (S44).
                let class = ClassId(arg(args, "class")?.into());
                self.change(arg(args, "show")?, "add_presentation", |state| prepare::add_presentation(state, &class))
            }
            "move_phase" => {
                let phase = ClassId(arg(args, "phase")?.into());
                let to = args["to"].as_u64().ok_or("missing argument to")? as usize;
                self.change(arg(args, "show")?, "move_phase", |state| prepare::move_phase(state, &phase, to))
            }
            // Changes: each returns what can now be undone.
            "set_up_class" => {
                let published = PublishedClassId(args["published"].as_u64().ok_or("missing argument published")?);
                let seed = self.random();
                let show = arg(args, "show")?;
                let mut answer = self.change(show, "set_up_class", |state| prepare::set_up(state, &published, seed))?;
                answer["class"] = json!(published.0.to_string());
                Ok(answer)
            }
            "set_up_following" => {
                let published = PublishedClassId(args["published"].as_u64().ok_or("missing argument published")?);
                let model = PublishedClassId(args["model"].as_u64().ok_or("missing argument model")?);
                let show = arg(args, "show")?;
                let mut answer = self.change(show, "set_up_following", |state| prepare::set_up_following(state, &published, &model))?;
                // The first section, to open.
                answer["class"] = self.with(show, |_, store| Ok(json!(store.state().classes.values().filter(|c| c.published == published).map(|c| c.id.0.clone()).min())))?;
                Ok(answer)
            }
            "place_entry" => {
                let (entry, class) = (EntryId(args["entry"].as_u64().ok_or("missing argument entry")?), ClassId(arg(args, "class")?.into()));
                self.change(arg(args, "show")?, "place_entry", |state| entries::place_entry(state, &entry, &class))
            }
            "set_scoring_table" => {
                let class = ClassId(arg(args, "class")?.into());
                let table = args["table"].as_u64().ok_or("missing argument table")? as u32;
                self.change(arg(args, "show")?, "set_scoring_table", |state| prepare::set_scoring_table(state, &class, table))
            }
            "class_details" => {
                let class = ClassId(arg(args, "class")?.into());
                self.with(arg(args, "show")?, |_, store| {
                    let state = store.state();
                    let c = state.classes.get(&class).ok_or_else(|| format!("unknown class {class}"))?;
                    // The show's days, and those its classes were moved to.
                    let planned = state.event.as_ref().map(|e| views::days_between(&e.from, &e.to)).unwrap_or_default();
                    let mut days: Vec<String> = planned.iter().cloned().chain(state.classes.values().map(|c| c.date.clone())).filter(|d| !d.is_empty()).collect();
                    days.sort();
                    days.dedup();
                    Ok(json!({
                        "number": c.number,
                        "date": c.date,
                        "sponsor": c.sponsor,
                        "placed_count": c.options.placed_count,
                        "max_starters": c.options.max_starters,
                        "extra_fields": c.extra_fields.clone().unwrap_or_default().iter().map(|f| json!({ "key": f.key, "value": f.value })).collect::<Vec<_>>(),
                        "logo": c.logo.as_ref().and_then(|l| l.as_str()),
                        "days": days,
                        "planned_days": planned,
                        // The ideal-time table: the office sets the ideal time (S20).
                        "ideal_table": results::ideal_table(state, c),
                        "ideal_time": c.options.ideal_time.map(prepare::time_text),
                        // The scoring table (jumping), changed only before results.
                        "scoring_table": c.scoring_table,
                        "scoring_tables": if c.scoring_table.is_some() { state.scoring_tables.values().map(|t| json!({ "id": t.id, "code": t.code, "name": t.name, "name_fr": t.name_fr, "name_it": t.name_it })).collect::<Vec<_>>() } else { Vec::new() },
                        "has_results": prepare::has_results(state, &class),
                        // The dressage test (dressage, a dressage phase), changed only before
                        // marks, those put aside included.
                        "programme": c.programme,
                        "uses_programme": c.programme.is_some() || state.class_discipline(c) == "CD" || c.phase.as_ref().is_some_and(|p| p.kind == show::model::PhaseKind::Dressage),
                        "programme_locked": prepare::has_results(state, &class) || state.aside.keys().any(|(k, _)| k == &class),
                        // Dressage: the judges give a total only (Rosson's manual entry).
                        "dressage": state.class_discipline(c) == "CD",
                        "totals_per_judge": c.options.totals_per_judge,
                        // Team dressage: five judges 3/2.
                        "team_dressage": state.published_classes.get(&c.published).is_some_and(|p| p.discipline == "CD" && p.team_size.is_some_and(|n| n > 0)),
                        "team_3_2": c.options.team_3_2,
                        // Team jumping: every rider's result counts, none dropped (S36).
                        "team_jumping": state.published_classes.get(&c.published).is_some_and(|p| p.discipline == "CS" && p.team_size.is_some_and(|n| n > 0)),
                        "team_count_all": c.options.team_count_all,
                        // Dressage: a Tag der Jugend final (its programme is one), no ex aequo in ranks 1–3 (S38).
                        "youth_programme": c.programme.and_then(|p| state.programmes.get(&p)).and_then(show::master::youth_final_movements).is_some(),
                        "youth_final": c.options.youth_final,
                        // Eventing: the phase (dressage, jumping, cross) or the class made of phases.
                        "phase": c.phase.as_ref().map(|p| p.kind),
                        "phases": !c.phases.is_empty(),
                        "ideal_time_penalties": c.options.ideal_time_penalties,
                        "max_time": c.options.max_time.map(prepare::time_text),
                        "max_time_penalties": c.options.max_time_penalties,
                        "rank_points": c.options.rank_points,
                        // Eventing: the rules decide, no choice (Eventing-Reglement 5.9).
                        "eventing": state.eventing(c),
                        "driving": c.options.driving,
                        // A full event's cones: Wertung A, the overtime to the hundredth (Fahrreglement 8.1.1, 8.4.4).
                        "driving_full_event": c.phase.as_ref().and_then(|p| state.classes.get(&p.of)).and_then(|parent| state.driving_event(parent)) == Some(show::model::DrivingEvent::Full),
                        // Driving: the class's licences and turnouts (cones' allowed times, bonuses).
                        "driving_licences": c.phase.as_ref().and_then(|p| state.classes.get(&p.of)).and_then(|k| state.published_classes.get(&k.published)).map(|p| p.record["fahren_lizenzen"].clone()).unwrap_or(Value::Null),
                        "driving_turnouts": c.phase.as_ref().and_then(|p| state.classes.get(&p.of)).and_then(|k| state.published_classes.get(&k.published)).map(|p| p.record["fahren_gespanne"].clone()).unwrap_or(Value::Null),
                        "published": views::published_info(state, &c.published),
                    }))
                })
            }
            "set_class_details" => {
                let class = ClassId(arg(args, "class")?.into());
                let text = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_string);
                // The number is the app's (the published class's, a letter per section); the day
                // and time are set with the name (edit_class).
                let details = prepare::Details {
                    sponsor: text("sponsor"),
                    placed_count: args.get("placed_count").map(|v| v.as_u64().map(|n| n as u32)),
                    extra_fields: args.get("extra_fields").and_then(Value::as_array).map(|a| {
                        a.iter().map(|f| show::model::ExtraField { key: f["key"].as_str().unwrap_or("").into(), value: f["value"].as_str().unwrap_or("").into() }).collect()
                    }),
                    logo: args.get("logo").map(|v| v.as_str().map(str::to_string)),
                    max_starters: args.get("max_starters").map(|v| v.as_u64().map(|n| n as u32)),
                    ideal_time: args.get("ideal_time").map(|v| v.as_str().map(str::to_string)),
                    scoring_table: args["scoring_table"].as_u64().map(|t| t as u32),
                    totals_per_judge: args["totals_per_judge"].as_bool(),
                    team_3_2: args["team_3_2"].as_bool(),
                    team_count_all: args["team_count_all"].as_bool(),
                    youth_final: args["youth_final"].as_bool(),
                    ideal_time_penalties: args["ideal_time_penalties"].as_bool(),
                    max_time: args.get("max_time").map(|v| v.as_str().map(str::to_string)),
                    max_time_penalties: args["max_time_penalties"].as_bool(),
                    rank_points: args["rank_points"].as_bool(),
                    driving: match args.get("driving") {
                        Some(d) => Some(serde_json::from_value(d.clone()).map_err(|e| format!("driving: {e}"))?),
                        None => None,
                    },
                };
                // The published class too (its category as printed, its levels): one change.
                let (category_text, levels) = (text("category_text"), args["levels"].as_u64().map(|n| n as u32));
                self.change(arg(args, "show")?, "set_class_details", |state| {
                    let published = state.classes.get(&class).ok_or_else(|| format!("unknown class {class}"))?.published.clone();
                    let mut changes = prepare::set_details(state, &class, details)?;
                    changes.extend(prepare::edit_published(state, &published, category_text, levels)?);
                    Ok(changes)
                })
            }
            "delete_results" => {
                let class = ClassId(arg(args, "class")?.into());
                self.change(arg(args, "show")?, "delete_results", |state| prepare::delete_results(state, &class))
            }
            "remove_start_lists" => {
                let published = PublishedClassId(args["published"].as_u64().ok_or("missing argument published")?);
                self.change(arg(args, "show")?, "remove_start_lists", |state| prepare::remove_start_lists(state, &published))
            }
            // Dividing a class into sections (U13): the plan, a rule's suggestion, applying it.
            "sections" => {
                let published = PublishedClassId(args["published"].as_u64().ok_or("missing argument published")?);
                self.with(arg(args, "show")?, |_, store| Ok(json_of(&sections::plan(store.state(), &published)?)))
            }
            "suggest_sections" => {
                let published = PublishedClassId(args["published"].as_u64().ok_or("missing argument published")?);
                let parts = args["parts"].as_u64().unwrap_or(2) as usize;
                let rule = match arg(args, "rule")? {
                    "balanced" => sections::Rule::Balanced,
                    "club" => sections::Rule::Club,
                    "town" => sections::Rule::Town,
                    "comment" => sections::Rule::Comment,
                    "level" => sections::Rule::Level,
                    "pony" => sections::Rule::Pony(
                        args["groups"].as_array().ok_or("missing argument groups")?.iter().map(|g| g.as_str().unwrap_or("").chars().map(|c| c.to_string()).collect()).collect(),
                    ),
                    other => return Err(format!("no rule called {other}")),
                };
                self.with(arg(args, "show")?, |_, store| Ok(json!(sections::suggest(&sections::plan(store.state(), &published)?, &rule, parts)?)))
            }
            "apply_sections" => {
                let published = PublishedClassId(args["published"].as_u64().ok_or("missing argument published")?);
                let parts = args["parts"].as_u64().ok_or("missing argument parts")? as usize;
                // For each rider of the plan, the section of each of their horses.
                let given: Vec<Vec<usize>> = args["sections"]
                    .as_array()
                    .ok_or("missing argument sections")?
                    .iter()
                    .map(|r| r.as_array().ok_or("a rider's horses' sections")?.iter().map(|s| s.as_u64().map(|s| s as usize).ok_or("a section is a number")).collect::<Result<Vec<_>, _>>())
                    .collect::<Result<_, _>>()?;
                let show = arg(args, "show")?;
                let now = now_millis();
                let mut answer = self.change(show, "apply_sections", |state| {
                    let event = state.event.as_ref().map_or(0, |e| e.id.0);
                    sections::apply(state, &published, &given, parts, &mut || se_api::load::assignment_id(event, now, self.random()))
                })?;
                answer["class"] = self.with(show, |_, store| Ok(json!(store.state().classes.values().filter(|c| c.published == published).map(|c| c.id.0.clone()).min())))?;
                Ok(answer)
            }
            "draw" => {
                let (class, seed) = (ClassId(arg(args, "class")?.into()), self.random());
                self.change(arg(args, "show")?, "draw", |state| prepare::draw(state, &class, seed))
            }
            "rearrange_rider" => {
                let class = ClassId(arg(args, "class")?.into());
                let entries: Vec<EntryId> = args["entries"].as_array().ok_or("missing argument entries")?.iter().map(|e| e.as_u64().map(EntryId).ok_or("an entry is a number")).collect::<Result<_, _>>()?;
                let follow = args["numbers_follow"] == true;
                self.change(arg(args, "show")?, "rearrange_rider", |state| prepare::rearrange_rider(state, &class, &entries, follow))
            }
            "move_start" => {
                let class = ClassId(arg(args, "class")?.into());
                let (entry, to) = (EntryId(args["entry"].as_u64().ok_or("missing argument entry")?), args["to"].as_u64().ok_or("missing argument to")? as usize);
                self.change(arg(args, "show")?, "move_start", |state| prepare::move_start(state, &class, &entry, to))
            }
            "number" => {
                let (class, first) = (ClassId(arg(args, "class")?.into()), args["first"].as_u64().unwrap_or(1) as u32);
                self.change(arg(args, "show")?, "number", |state| prepare::number(state, &class, first))
            }
            "split" => {
                let (class, parts) = (ClassId(arg(args, "class")?.into()), args["parts"].as_u64().unwrap_or(2) as usize);
                self.change(arg(args, "show")?, "split", |state| prepare::split(state, &class, parts))
            }
            "move_entry" => {
                let entry = EntryId(args["entry"].as_u64().ok_or("missing argument entry")?);
                let (from, to) = (ClassId(arg(args, "from")?.into()), ClassId(arg(args, "to")?.into()));
                self.change(arg(args, "show")?, "move_entry", |state| prepare::move_entry(state, &entry, &from, &to))
            }
            "edit_entry" => {
                let entry = EntryId(args["entry"].as_u64().ok_or("missing argument entry")?);
                let (ns, hc) = (args["not_starting"].as_bool(), args["hors_concours"].as_bool());
                self.change(arg(args, "show")?, "edit_entry", |state| prepare::edit_entry(state, &entry, ns, hc))
            }
            "set_horse_phases" => {
                let entry = EntryId(args["entry"].as_u64().ok_or("missing argument entry")?);
                let horse = args["horse"].as_u64().ok_or("missing argument horse")? as usize;
                let phases: Vec<String> = args["phases"].as_array().ok_or("missing argument phases")?.iter().filter_map(|p| p.as_str().map(str::to_string)).collect();
                self.change(arg(args, "show")?, "set_horse_phases", |state| entries::set_horse_phases(state, &entry, horse, &phases))
            }
            "change_horse" => {
                let entries = entries_arg(args)?;
                let pick = self.horse_pick(args)?;
                self.change(arg(args, "show")?, "change_horse", |state| prepare::change_horse(state, &entries, &pick))
            }
            "edit_class" => {
                let class = ClassId(arg(args, "class")?.into());
                let text = |k: &str| args[k].as_str().map(str::to_string);
                let edit = prepare::ClassEdit {
                    name: text("name"),
                    date: text("date"),
                    time: text("time"),
                    programme: args.get("programme").map(|p| p.as_u64().map(|p| p as u32)),
                    qualifier: args.get("qualifier").map(|q| q.as_str().map(|q| ClassId(q.into()))),
                    scoring_table: args["scoring_table"].as_u64().map(|t| t as u32),
                };
                self.change(arg(args, "show")?, "edit_class", |state| prepare::edit_class(state, &class, edit))
            }
            // Entries made or changed at the show.
            "add_entry" => {
                // One published class, or several (`published` a list: the same rider and horse in
                // each, one change; team classes one at a time).
                let published: Vec<PublishedClassId> = match &args["published"] {
                    Value::Array(list) => list.iter().map(|p| p.as_u64().map(PublishedClassId).ok_or_else(|| "published: numbers".to_string())).collect::<Result<_, _>>()?,
                    p => vec![PublishedClassId(p.as_u64().ok_or("missing argument published")?)],
                };
                if published.is_empty() {
                    return Err("no class to enter".into());
                }
                let (rider, horse) = (self.rider_pick(&args["rider"])?, self.horse_pick(&args["horse"])?);
                // Team classes: `{"team": id}` to join one, `{"new_team": name}` to make one.
                let team = match (args["team"].as_str(), args["new_team"].as_str()) {
                    (Some(t), _) => Some(entries::TeamPick::Join(show::model::TeamId(t.into()))),
                    (None, Some(name)) => Some(entries::TeamPick::New(name.into())),
                    _ => None,
                };
                let show = arg(args, "show")?;
                let mut ids: Vec<u64> = Vec::new();
                let mut answer = self.change(show, "add_entry", |state| {
                    if published.len() > 1 && published.iter().any(|p| state.published_classes.get(p).is_some_and(|p| p.team_size.is_some())) {
                        return Err("team classes: one at a time (each its team)".into());
                    }
                    let mut changes: Vec<show::oplog::Change> = Vec::new();
                    // Rosson's ids for entries made at the show: the time in milliseconds.
                    let mut id = now_millis();
                    for p in &published {
                        while state.entries.contains_key(&EntryId(id)) || ids.contains(&id) {
                            id += 1;
                        }
                        ids.push(id);
                        // The rider or horse copied from the registry once.
                        for c in entries::add_entry(state, p, &rider, &horse, team.as_ref(), EntryId(id))? {
                            if !changes.contains(&c) {
                                changes.push(c);
                            }
                        }
                    }
                    Ok(changes)
                })?;
                answer["entry"] = json!(ids.first());
                answer["entries"] = json!(ids);
                Ok(answer)
            }
            "teams" => {
                let published = PublishedClassId(args["published"].as_u64().ok_or("missing argument published")?);
                self.with(arg(args, "show")?, |_, store| {
                    let state = store.state();
                    let size = state.published_classes.get(&published).and_then(|p| p.team_size);
                    let mut teams: Vec<Value> = entries::teams_of(state, &published)
                        .into_iter()
                        .map(|(id, riders)| json!({ "id": id.0, "name": state.teams.get(&id).map(|t| t.name.clone()).unwrap_or_default(), "riders": riders }))
                        .collect();
                    teams.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
                    Ok(json!({ "size": size, "teams": teams }))
                })
            }
            "move_team" => {
                let (class, team) = (ClassId(arg(args, "class")?.into()), show::model::TeamId(arg(args, "team")?.into()));
                // Dragged to a place (`to`, from 0), or a step earlier or later.
                match args["to"].as_u64() {
                    Some(to) => self.change(arg(args, "show")?, "move_team", |state| prepare::move_team_to(state, &class, &team, to as usize)),
                    None => {
                        let step = args["step"].as_i64().ok_or("missing argument step")? as i32;
                        self.change(arg(args, "show")?, "move_team", |state| prepare::move_team(state, &class, &team, step))
                    }
                }
            }
            "team_not_starting" => {
                let team = show::model::TeamId(arg(args, "team")?.into());
                let not_starting = args["not_starting"].as_bool().ok_or("missing argument not_starting")?;
                self.change(arg(args, "show")?, "team_not_starting", |state| prepare::team_not_starting(state, &team, not_starting))
            }
            "rename_team" => {
                let team = show::model::TeamId(arg(args, "team")?.into());
                let name = arg(args, "name")?.trim().to_string();
                self.change(arg(args, "show")?, "rename_team", |state| {
                    let t = state.teams.get(&team).ok_or_else(|| format!("unknown team {team}"))?;
                    if name.is_empty() {
                        return Err("a team needs a name".into());
                    }
                    Ok(if t.name == name { Vec::new() } else { vec![show::oplog::Change::TeamSet { team: show::model::Team { name: name.clone(), ..t.clone() } }] })
                })
            }
            // A rider's address and contacts, as Swiss Equestrian delivered them: read only (the
            // office never changes a rider's data).
            "address" => {
                let person = show::model::PersonId(args["person"].as_u64().ok_or("missing argument person")?);
                self.with(arg(args, "show")?, |_, store| {
                    let state = store.state();
                    let p = state.persons.get(&person).ok_or_else(|| format!("unknown person {person}"))?;
                    let c = state.contacts.get(&person).cloned().unwrap_or_default();
                    Ok(json!({ "town": p.town, "street": c.street, "postcode": c.postcode, "email": c.email, "phone_mobile": c.phone_mobile, "phone_private": c.phone_private }))
                })
            }
            "remove_entry" => {
                let entry = EntryId(args["entry"].as_u64().ok_or("missing argument entry")?);
                self.change(arg(args, "show")?, "remove_entry", |state| entries::remove_entry(state, &entry))
            }
            "change_rider" => {
                let entries = entries_arg(args)?;
                let rider = self.rider_pick(&args["rider"])?;
                self.change(arg(args, "show")?, "change_rider", |state| entries::change_rider(state, &entries, &rider))
            }
            "move_to_class" => {
                let entry = EntryId(args["entry"].as_u64().ok_or("missing argument entry")?);
                let to = PublishedClassId(args["published"].as_u64().ok_or("missing argument published")?);
                self.change(arg(args, "show")?, "move_to_class", |state| entries::move_to_class(state, &entry, &to))
            }
            "edit_entry_details" => {
                let entry = EntryId(args["entry"].as_u64().ok_or("missing argument entry")?);
                let (stage, comment) = (args["stage"].as_i64(), args["comment"].as_str().map(str::to_string));
                self.change(arg(args, "show")?, "edit_entry_details", |state| entries::edit_details(state, &entry, stage, comment))
            }
            // Riders and horses to pick: the show's first, then Swiss Equestrian's registry.
            "find_riders" => {
                let query = args["query"].as_str().unwrap_or("").to_string();
                let mut found = self.with(arg(args, "show")?, |_, store| Ok(entries::find_riders(store.state(), &query)))?;
                let known: std::collections::HashSet<u64> = found.iter().filter_map(|f| f["person"].as_u64()).collect();
                for record in self.registry.find_persons(&query, 40) {
                    let Some(id) = se_api::load::number(&record["id"]).filter(|id| !known.contains(id)) else { continue };
                    if found.len() >= 20 {
                        break;
                    }
                    found.push(entries::rider_found(&se_api::load::person_of(id, &record).0, false));
                }
                Ok(json!({ "found": found, "registry": self.registry.about() }))
            }
            // `entry`: whose rider may ride a guest horse (`guest`).
            "find_horses" => {
                let query = args["query"].as_str().unwrap_or("").to_string();
                let entry = args["entry"].as_u64().map(EntryId);
                let (mut found, year, guest) = self.with(arg(args, "show")?, |_, store| {
                    let state = store.state();
                    let year = entries::show_year(state);
                    let guest = entry.as_ref().and_then(|e| state.entries.get(e)).and_then(|e| state.persons.get(&e.rider)).is_some_and(|p| entries::guest_licence(&p.licences));
                    Ok((entries::find_horses(state, &query, year), year, guest))
                })?;
                let known: std::collections::HashSet<u64> = found.iter().filter_map(|f| f["horse"].as_u64()).collect();
                for record in self.registry.find_horses(&query, 40) {
                    let Some(id) = se_api::load::number(&record["id"]).filter(|id| !known.contains(id) && *id != se_api::load::PLACEHOLDER_HORSE) else { continue };
                    if found.len() >= 20 {
                        break;
                    }
                    found.push(entries::horse_found(&se_api::load::horse_of(id, &record), false, year));
                }
                Ok(json!({ "found": found, "guest": guest, "year": year, "registry": self.registry.about() }))
            }
            "registry" => Ok(json_of(&self.registry.about())),
            // Swiss Equestrian's registry, through the show's connection (its server, its key).
            "registry_download" => self.download_registry(arg(args, "show")?),
            "published" => self.with(arg(args, "show")?, |_, store| {
                let state = store.state();
                let mut list: Vec<Value> = state
                    .published_classes
                    .values()
                    .map(|p| json!({ "id": p.id.0, "number": p.number, "discipline": p.discipline, "category": p.category_text, "date": p.date, "teams": p.team_size.is_some() }))
                    .collect();
                list.sort_by_key(|p| (p["date"].as_str().unwrap_or("").to_string(), p["number"].as_str().and_then(|n| n.parse::<u32>().ok()).unwrap_or(u32::MAX)));
                Ok(Value::Array(list))
            }),
            // A class's results made final (definitiv), or reopened with a reason.
            "finalise" => {
                let class = ClassId(arg(args, "class")?.into());
                self.change(arg(args, "show")?, "finalise", |state| {
                    let c = state.classes.get(&class).ok_or_else(|| format!("unknown class {class}"))?;
                    if c.status == show::model::ClassStatus::Final {
                        return Ok(Vec::new());
                    }
                    Ok(vec![state.finalise(&class)])
                })
            }
            "reopen" => {
                let class = ClassId(arg(args, "class")?.into());
                let reason = args["reason"].as_str().unwrap_or("").trim().to_string();
                self.change(arg(args, "show")?, "reopen", |state| {
                    let c = state.classes.get(&class).ok_or_else(|| format!("unknown class {class}"))?;
                    // A part of a driving class is final with the whole class (`State::status_of`):
                    // reopening the part reopens what holds it, the class and the part.
                    let holding: Vec<ClassId> = std::iter::once(class.clone())
                        .chain(c.phase.as_ref().map(|p| p.of.clone()))
                        .filter(|id| state.classes.get(id).is_some_and(|c| c.status == show::model::ClassStatus::Final))
                        .collect();
                    if holding.is_empty() {
                        return Ok(Vec::new());
                    }
                    if reason.is_empty() {
                        return Err("say why the class is reopened".into());
                    }
                    Ok(holding.into_iter().map(|class| show::oplog::Change::ClassReopened { class, reason: reason.clone() }).collect())
                })
            }
            // What two computers changed apart (R2, R4, R7, R11): looked at, or the other version
            // put back.
            "conflicts" => self.with(arg(args, "show")?, |_, store| {
                let seen = conflicts::seen_ids(store.meta(conflicts::META_SEEN).map_err(|e| e.0)?);
                let log = store.log().map_err(|e| e.0)?;
                let list = conflicts::conflicts(&log, store.state(), &self.device, &seen);
                Ok(json!({ "unseen": list.iter().filter(|c| !c.seen).count(), "conflicts": list }))
            }),
            "acknowledge_conflicts" => {
                let ids: Vec<String> = args["ids"].as_array().ok_or("missing argument ids")?.iter().filter_map(|i| i.as_str().map(str::to_string)).collect();
                self.acknowledge(arg(args, "show")?, &ids)?;
                Ok(json!({ "ok": true }))
            }
            "restore_conflict" => {
                let (show, id) = (arg(args, "show")?, arg(args, "id")?);
                let (log, before) = self.with(show, |_, store| Ok((store.log().map_err(|e| e.0)?, store.state().conflicts.iter().map(|c| c.id.clone()).collect::<std::collections::BTreeSet<_>>())))?;
                let answer = self.change(show, "restore_conflict", |state| conflicts::restore(&log, state, id))?;
                // Looked at: this one, and those the version put back makes (others see them).
                let mut ids = vec![id.to_string()];
                ids.extend(self.with(show, |_, store| Ok(store.state().conflicts.iter().filter(|c| !before.contains(&c.id)).map(|c| c.id.clone()).collect::<Vec<_>>()))?);
                self.acknowledge(show, &ids)?;
                Ok(answer)
            }
            "history" => {
                let lang = prints::Language::of(args["language"].as_str().unwrap_or("fr"));
                let query = args["query"].as_str().unwrap_or("").to_string();
                let limit = args["limit"].as_u64().unwrap_or(300) as usize;
                self.with(arg(args, "show")?, |_, store| {
                    let log = store.log().map_err(|e| e.0)?;
                    Ok(json_of(&history::history(&log, store.state(), lang, &query, limit)))
                })
            }
            // Prizes of a published class; `scale`: the jumping scale applies (show jumping only).
            "prizes" => {
                let published = PublishedClassId(args["published"].as_u64().ok_or("missing argument published")?);
                self.with(arg(args, "show")?, |_, store| {
                    let state = store.state();
                    let fee = state.published_classes.get(&published).and_then(|p| p.fee);
                    let scale = results::prize_scale_applies(state, &published);
                    Ok(json!({ "prizes": results::prizes_or_default(state, &published), "set": state.prizes.contains_key(&published), "fee": fee, "scale": scale }))
                })
            }
            "prize_scale" => {
                let first = number(&args["first"])?.unwrap_or_default();
                let minimum = number(&args["minimum"])?.unwrap_or_default();
                let ranks = args["ranks"].as_u64().unwrap_or(10).min(30) as usize;
                Ok(json!(results::prize_scale(first, minimum, ranks)))
            }
            "set_prizes" => {
                let published = PublishedClassId(args["published"].as_u64().ok_or("missing argument published")?);
                let prizes: Option<show::model::Prizes> = if args["prizes"].is_null() { None } else { Some(serde_json::from_value(args["prizes"].clone()).map_err(|e| format!("prizes: {e}"))?) };
                self.change(arg(args, "show")?, "set_prizes", |state| results::set_prizes(state, &published, prizes))
            }
            // Start numbers fixed for the whole event.
            "set_fixed_numbers" => {
                let fixed = args["fixed"].as_bool().ok_or("missing argument fixed")?;
                self.change(arg(args, "show")?, "set_fixed_numbers", |state| prepare::set_fixed_numbers(state, fixed))
            }
            "number_show" => {
                let order = match args["order"].as_str().unwrap_or("name") {
                    "horse" => prepare::NumberOrder::Horse,
                    "start_order" => prepare::NumberOrder::StartOrder,
                    "random" => prepare::NumberOrder::Random,
                    "winning_points" => prepare::NumberOrder::WinningPoints,
                    _ => prepare::NumberOrder::Name,
                };
                let (descending, seed) = (args["descending"].as_bool().unwrap_or(false), self.random());
                self.change(arg(args, "show")?, "number_show", |state| prepare::number_show(state, order, descending, seed))
            }
            "reorder" => {
                let class = ClassId(arg(args, "class")?.into());
                let how = match arg(args, "by")? {
                    "name" => prepare::Reorder::Name,
                    "horse" => prepare::Reorder::Horse,
                    "winning_points" => prepare::Reorder::WinningPoints,
                    "follow" => prepare::Reorder::Follow(ClassId(arg(args, "other")?.into())),
                    "ranking" => prepare::Reorder::Ranking(ClassId(arg(args, "other")?.into())),
                    "from" => prepare::Reorder::From(EntryId(args["entry"].as_u64().ok_or("missing argument entry")?)),
                    other => return Err(format!("no start order called {other}")),
                };
                let descending = args["descending"].as_bool().unwrap_or(false);
                self.change(arg(args, "show")?, "reorder", |state| prepare::reorder(state, &class, &how, descending))
            }
            "move_class" => {
                let class = ClassId(arg(args, "class")?.into());
                // Dragged to a place of its day or of another (`date`; `to`, from 0), or a step
                // earlier or later.
                match args["to"].as_u64() {
                    Some(to) => self.change(arg(args, "show")?, "move_class", |state| prepare::move_class_to(state, &class, args["date"].as_str(), to as usize)),
                    None => {
                        let step = args["step"].as_i64().unwrap_or(1).clamp(-1, 1) as i32;
                        self.change(arg(args, "show")?, "move_class", |state| prepare::move_class(state, &class, step))
                    }
                }
            }
            // Excel files of the show.
            "export_excel" => {
                let lang = prints::Language::of(args["language"].as_str().unwrap_or("fr"));
                let path = PathBuf::from(arg(args, "path")?);
                let bytes = self.with(arg(args, "show")?, |_, store| {
                    let state = store.state();
                    let class = || arg(args, "class").map(|c| ClassId(c.into()));
                    match arg(args, "what")? {
                        "participants" => exports::participants(state, lang),
                        "entries" => exports::entries(state, lang),
                        "start_list" => exports::start_list(state, &class()?, lang),
                        "ranking" => exports::ranking(state, &class()?, lang),
                        other => Err(format!("nothing to export called {other}")),
                    }
                })?;
                std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
                Ok(json!({ "path": path }))
            }
            "set_layout" => {
                let class = ClassId(arg(args, "class")?.into());
                let setup: show::model::StartListSetup = serde_json::from_value(args["setup"].clone()).map_err(|e| format!("layout: {e}"))?;
                self.change(arg(args, "show")?, "set_layout", |state| prepare::set_layout(state, &class, setup))
            }
            // One official to several classes at once (the same role, or each class's own).
            "set_roles" => {
                let person = show::model::PersonId(args["person"].as_u64().ok_or("missing argument person")?);
                // Each class, and its role (code, judge's letter) or none.
                type Role = Option<(String, Option<String>)>;
                let items: Vec<(ClassId, Role)> = args["roles"]
                    .as_array()
                    .ok_or("missing argument roles")?
                    .iter()
                    .map(|r| Ok((ClassId(r["class"].as_str().ok_or("a role without its class")?.into()), r["role"].as_str().map(|code| (code.to_string(), r["letter"].as_str().map(str::to_string))))))
                    .collect::<Result<_, String>>()?;
                let show = arg(args, "show")?;
                let now = now_millis();
                self.change(show, "set_roles", |state| {
                    let event = state.event.as_ref().map_or(0, |e| e.id.0);
                    let mut changes = Vec::new();
                    for (class, role) in &items {
                        let role = role.as_ref().map(|(code, letter)| (code.as_str(), letter.as_deref()));
                        changes.extend(prepare::set_role(state, class, &person, role, now, &mut || se_api::load::assignment_id(event, now, self.random()))?);
                    }
                    Ok(changes)
                })
            }
            "set_role" => {
                let class = ClassId(arg(args, "class")?.into());
                let person = show::model::PersonId(args["person"].as_u64().ok_or("missing argument person")?);
                let (role, letter) = (args["role"].as_str().map(str::to_string), args["letter"].as_str().map(str::to_string));
                let show = arg(args, "show")?;
                let now = now_millis();
                self.change(show, "set_role", |state| {
                    let event = state.event.as_ref().map_or(0, |e| e.id.0);
                    prepare::set_role(state, &class, &person, role.as_deref().map(|r| (r, letter.as_deref())), now, &mut || se_api::load::assignment_id(event, now, self.random()))
                })
            }
            "officials" => self.with(arg(args, "show")?, |_, store| Ok(json_of(&views::officials(store.state())))),
            "event_officials" => {
                let language = args["language"].as_str().unwrap_or("fr").to_string();
                self.with(arg(args, "show")?, |_, store| Ok(json_of(&views::event_officials(store.state(), &language))))
            }
            "set_event_role" => {
                let person = show::model::PersonId(args["person"].as_u64().ok_or("missing argument person")?);
                let role_id = args["role"].as_u64().ok_or("missing argument role (its id)")? as u32;
                let assignment = args["assignment"].as_str().map(|a| show::model::AssignmentId(a.into()));
                let (days, responsible) = (args["days"].as_u64().map(|d| d as u32), args["responsible"].as_bool().unwrap_or(false));
                let show = arg(args, "show")?;
                let now = now_millis();
                self.change(show, "set_event_role", |state| {
                    let event = state.event.as_ref().map_or(0, |e| e.id.0);
                    prepare::set_event_role(state, assignment.as_ref(), &person, role_id, days, responsible, now, &mut || se_api::load::assignment_id(event, now, self.random()))
                })
            }
            "remove_event_role" => {
                let assignment = show::model::AssignmentId(arg(args, "assignment")?.into());
                self.change(arg(args, "show")?, "remove_event_role", |state| prepare::remove_event_role(state, &assignment))
            }
            "find_officials" => {
                let query = args["query"].as_str().unwrap_or("").to_string();
                self.with(arg(args, "show")?, |_, store| Ok(json_of(&views::find_officials(store.state(), &query))))
            }
            "multitest" => {
                let class = ClassId(arg(args, "class")?.into());
                self.with(arg(args, "show")?, |_, store| Ok(json_of(&views::multitest(store.state(), &class)?)))
            }
            "sheets" => {
                let class = ClassId(arg(args, "class")?.into());
                self.with(arg(args, "show")?, |_, store| Ok(json_of(&views::sheets(store.state(), &class)?)))
            }
            "edit_sheet" => {
                let class = ClassId(arg(args, "class")?.into());
                // Team dressage: the team's sheet (`team`) instead of a rider's (`entry`).
                let team = args["team"].as_str().map(|t| show::model::TeamId(t.into()));
                let entry = EntryId(args["entry"].as_u64().or(team.as_ref().map(|_| 0)).ok_or("missing argument entry")?);
                let judge = show::model::AssignmentId(arg(args, "judge")?.into());
                let mut marks = std::collections::BTreeMap::new();
                for (k, v) in args["marks"].as_object().into_iter().flatten() {
                    let value = match v {
                        Value::Null => None,
                        Value::Number(n) => Some(scoring::number::parse(&n.to_string()).ok().flatten().ok_or_else(|| format!("movement {k}: not a number"))?),
                        Value::String(s) if s.trim().is_empty() => None,
                        Value::String(s) => Some(scoring::number::parse(s.trim()).ok().flatten().ok_or_else(|| format!("movement {k}: {s} is not a number"))?),
                        _ => return Err(format!("movement {k}: not a number")),
                    };
                    marks.insert(k.clone(), value);
                }
                let penalties = args["penalties"].as_array().map(|a| a.iter().filter_map(|i| i.as_u64().map(|i| i as usize)).collect());
                // Totals per judge: the judge's total (null clears it).
                let total = match args.get("total") {
                    None => None,
                    Some(Value::Null) => Some(None),
                    Some(v) => Some(Some(scoring::number::parse(v.as_str().map(str::trim).unwrap_or(&v.to_string())).ok().flatten().ok_or("the total is not a number")?)),
                };
                if let Some(team) = team {
                    return self.change(arg(args, "show")?, "edit_sheet", |state| prepare::edit_team_sheet(state, &class, &team, &judge, marks, penalties));
                }
                self.change(arg(args, "show")?, "edit_sheet", |state| prepare::edit_sheet(state, &class, &entry, &judge, marks, penalties, total))
            }
            "set_jury_penalty" => {
                // Eventing: the jury's penalties for a rider in a phase (S41); null takes them back.
                let class = ClassId(arg(args, "class")?.into());
                let entry = EntryId(args["entry"].as_u64().ok_or("missing argument entry")?);
                let points = match args.get("points") {
                    None | Some(Value::Null) => None,
                    Some(v) => Some(scoring::number::parse(v.as_str().map(str::trim).unwrap_or(&v.to_string())).ok().flatten().ok_or("the penalty is not a number")?),
                };
                self.change(arg(args, "show")?, "set_jury_penalty", |state| prepare::set_jury_penalty(state, &class, &entry, points))
            }
            "set_penalties" => {
                // One set for the rider (`entry`) or the team (`team`), on every judge's sheet (S40).
                let class = ClassId(arg(args, "class")?.into());
                let who = match args["team"].as_str() {
                    Some(t) => show::model::Competitor::Team(show::model::TeamId(t.into())),
                    None => show::model::Competitor::Entry(EntryId(args["entry"].as_u64().ok_or("missing argument entry")?)),
                };
                let places: Vec<usize> = args["penalties"].as_array().ok_or("missing argument penalties")?.iter().filter_map(|i| i.as_u64().map(|i| i as usize)).collect();
                self.change(arg(args, "show")?, "edit_sheet", |state| prepare::set_penalties(state, &class, &who, places))
            }
            "move_sheet" => {
                let class = ClassId(arg(args, "class")?.into());
                let place = |v: &Value| -> Option<show::model::SheetPlace> {
                    if let Some(slot) = v["aside"].as_u64() {
                        return Some(show::model::SheetPlace::Aside { slot: slot as u32 });
                    }
                    Some(show::model::SheetPlace::Rider { entry: EntryId(v["entry"].as_u64()?), judge: show::model::AssignmentId(v["judge"].as_str()?.into()) })
                };
                let from = place(&args["from"]).ok_or("missing argument from")?;
                let to = place(&args["to"]);
                // Cut (no `to`: put aside), pasted (from aside), or sent: what undo names.
                let what = match (&from, &to) {
                    (_, None) => "cut_sheet",
                    (show::model::SheetPlace::Aside { .. }, _) => "paste_sheet",
                    _ => "move_sheet",
                };
                self.change(arg(args, "show")?, what, |state| prepare::move_sheet(state, &class, from, to))
            }
            "delete_sheet" => {
                // A rider's sheet (`entry`, `judge`), a team's (`team`, `judge`) or a cut one (`aside`).
                let class = ClassId(arg(args, "class")?.into());
                let judge = args["judge"].as_str().map(|j| show::model::AssignmentId(j.into()));
                let show = arg(args, "show")?;
                if let Some(slot) = args["aside"].as_u64() {
                    return self.change(show, "delete_sheet", |state| prepare::delete_sheet(state, &class, show::model::SheetPlace::Aside { slot: slot as u32 }));
                }
                let judge = judge.ok_or("missing argument judge")?;
                if let Some(team) = args["team"].as_str() {
                    let team = show::model::TeamId(team.into());
                    return self.change(show, "delete_sheet", |state| prepare::delete_team_sheet(state, &class, &team, &judge));
                }
                let entry = EntryId(args["entry"].as_u64().ok_or("missing argument entry")?);
                self.change(show, "delete_sheet", |state| prepare::delete_sheet(state, &class, show::model::SheetPlace::Rider { entry, judge }))
            }
            "undo" => self.undo(arg(args, "show")?),
            "redo" => self.redo(arg(args, "show")?),
            // The tests; for a class (`class`), those of its discipline (Rosson filters them so).
            "programmes" => self.with(arg(args, "show")?, |_, store| {
                let state = store.state();
                let class = args["class"].as_str().and_then(|c| state.classes.get(&ClassId(c.into())));
                let discipline = class.and_then(|c| state.published_classes.get(&c.published)).map(|p| p.discipline_id);
                // Eventing's dressage phase: the dressage tests too (Swiss Equestrian's discipline 2).
                let phase = class.is_some_and(|c| c.phase.is_some());
                let mut list: Vec<Value> = state.programmes.values().filter(|p| discipline.is_none_or(|d| p.discipline_id.is_none_or(|x| x == d || (phase && x == 2)))).map(|p| json!({ "id": p.id, "name": p.name, "title": show::master::programme_title(p, "fr"), "titel": show::master::programme_title(p, "de"), "titolo": show::master::programme_title(p, "it") })).collect();
                list.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
                Ok(Value::Array(list))
            }),
            "horses" => self.with(arg(args, "show")?, |_, store| Ok(json_of(&views::horses(store.state())))),
            // Show-jumping results.
            "results" => {
                let class = ClassId(arg(args, "class")?.into());
                self.with(arg(args, "show")?, |_, store| Ok(json_of(&views::results(store.state(), &class)?)))
            }
            "set_rank" => {
                let class = ClassId(arg(args, "class")?.into());
                let who = match (args["team"].as_str(), args["entry"].as_u64()) {
                    (Some(t), _) => show::model::Competitor::Team(show::model::TeamId(t.into())),
                    (None, Some(e)) => show::model::Competitor::Entry(EntryId(e)),
                    _ => return Err("missing argument entry or team".into()),
                };
                let rank = args["rank"].as_u64().map(|r| r as u32);
                let reason = args["reason"].as_str().unwrap_or("set by hand").to_string();
                self.change(arg(args, "show")?, "set_rank", |state| results::set_rank(state, &class, who.clone(), rank, &reason))
            }
            "edit_result" if args["team"].is_string() => {
                let class = ClassId(arg(args, "class")?.into());
                let team = show::model::TeamId(arg(args, "team")?.into());
                let out = args["out"].as_u64().map(|o| o as u32);
                self.change(arg(args, "show")?, "edit_result", |state| results::edit_team_out(state, &class, &team, out))
            }
            "edit_result" => {
                let class = ClassId(arg(args, "class")?.into());
                let entry = EntryId(args["entry"].as_u64().ok_or("missing argument entry")?);
                let mut values = std::collections::BTreeMap::new();
                for (k, v) in args["values"].as_object().into_iter().flatten() {
                    // A time may be typed in minutes and seconds ("5:12", cross-country); a marathon's
                    // start or finish as the time of day ("10:12:30").
                    let clock = k.ends_with("_start") || k.ends_with("_finish");
                    let value = match v.as_str().filter(|t| t.contains(':')) {
                        Some(t) if clock => Some(prepare::parse_clock(t).map_err(|e| format!("{k}: {e}"))?),
                        Some(t) => Some(prepare::parse_time(t).map_err(|e| format!("{k}: {e}"))?),
                        None => number(v).map_err(|e| format!("{k}: {e}"))?,
                    };
                    values.insert(k.clone(), value);
                }
                let out = args.get("out").map(|o| o.as_u64().map(|o| o as u32));
                self.change(arg(args, "show")?, "edit_result", |state| results::edit_result(state, &class, &entry, values, out))
            }
            "timekeeper_file" => {
                let class = ClassId(arg(args, "class")?.into());
                let path = PathBuf::from(arg(args, "path")?);
                let bytes = self.with(arg(args, "show")?, |_, store| {
                    let state = store.state();
                    let number = state.classes.get(&class).map(|c| c.number.clone()).unwrap_or_default();
                    let rows = timing::start_list(state, &class).map_err(|e| e.0)?;
                    if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("txt")) {
                        Ok(timing::to_text(&rows))
                    } else {
                        timing::to_xlsx(&rows, &number).map_err(|e| e.0)
                    }
                })?;
                std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
                Ok(json!({ "path": path }))
            }
            "import_results" => {
                let class = ClassId(arg(args, "class")?.into());
                let path = PathBuf::from(arg(args, "path")?);
                let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                let adopt = args["adopt_ranks"].as_bool().unwrap_or(false);
                let mut imported = results::Imported::default();
                let mut answer = self.change(arg(args, "show")?, "import_results", |state| {
                    let (changes, i) = results::import_results(state, &class, &bytes, adopt)?;
                    imported = i;
                    Ok(changes)
                })?;
                answer["results"] = json!(imported.results);
                answer["notes"] = json!(imported.notes);
                Ok(answer)
            }
            "reverse_order" => {
                let class = ClassId(arg(args, "class")?.into());
                self.change(arg(args, "show")?, "reverse_order", |state| results::reverse_of_qualifier(state, &class))
            }
            // Rankings over several classes (U8).
            "combined" => {
                let id = arg(args, "id")?.to_string();
                self.with(arg(args, "show")?, |_, store| Ok(json_of(&views::combined(store.state(), &id)?)))
            }
            // The number a ranking over these classes gets (shown, never typed).
            "combined_number" => {
                let classes: Vec<ClassId> = args["classes"].as_array().ok_or("missing argument classes")?.iter().filter_map(|c| c.as_str().map(|c| ClassId(c.into()))).collect();
                let id = args["id"].as_str().map(str::to_string);
                self.with(arg(args, "show")?, |_, store| Ok(json!(store.state().next_combined_number(id.as_deref(), &classes))))
            }
            "set_combined" => {
                let id = args["id"].as_str().map_or_else(|| format!("{}-{}", self.device, now_millis()), str::to_string);
                let classes: Vec<ClassId> = args["classes"].as_array().ok_or("missing argument classes")?.iter().filter_map(|c| c.as_str().map(|c| ClassId(c.into()))).collect();
                let method = match arg(args, "method")? {
                    // Its tie-break: none (ex aequo, the default), or the penalties or the result
                    // in a class (`tie_break`).
                    "penalties" => show::model::CombinedMethod::Penalties {
                        tie: match args["tie"].as_str().unwrap_or("none") {
                            "none" => show::model::PenaltiesTie::None,
                            "faults" => show::model::PenaltiesTie::Faults { class: ClassId(arg(args, "tie_break")?.into()) },
                            "result" => show::model::PenaltiesTie::Result { class: ClassId(arg(args, "tie_break")?.into()) },
                            other => return Err(format!("unknown tie-break {other}")),
                        },
                    },
                    "ranks" => show::model::CombinedMethod::Ranks { tie_break: ClassId(arg(args, "tie_break")?.into()) },
                    "percentages" => show::model::CombinedMethod::Percentages { tie_break: args["tie_break"].as_str().map(|c| ClassId(c.into())) },
                    other => return Err(format!("unknown way of ranking {other}")),
                };
                let name = arg(args, "name")?.to_string();
                let mut answer = self.change(arg(args, "show")?, "set_combined", |state| {
                    // Changed: what the screen does not show stays (Rosson's record, extra lines).
                    let before = state.combined.get(&id);
                    // Never typed: kept while its classes stay, else the highest class's.
                    let number = Some(state.next_combined_number(before.map(|_| id.as_str()), &classes));
                    let ranking = show::model::CombinedRanking {
                        id: id.clone(),
                        name,
                        classes,
                        method,
                        number,
                        extra_fields: before.map(|b| b.extra_fields.clone()).unwrap_or_default(),
                        record: before.map(|b| b.record.clone()).unwrap_or_default(),
                    };
                    results::set_combined(state, ranking)
                })?;
                answer["id"] = json!(id);
                Ok(answer)
            }
            "remove_combined" => {
                let id = arg(args, "id")?.to_string();
                self.change(arg(args, "show")?, "remove_combined", |state| results::remove_combined(state, &id))
            }
            // Swiss Equestrian.
            "connection" => self.connection(arg(args, "show")?),
            "set_connection" => {
                // The event's access key (the server is the app's, not the show's).
                let show = arg(args, "show")?;
                if let Some(key) = args.get("access_key") {
                    let event = self.event_id(show)?;
                    let key = key.as_str().map(str::trim).filter(|k| !k.is_empty());
                    self.secrets.set(&connection::access_key_name(event), key)?;
                }
                self.connection(show)
            }
            "sending" => self.with(arg(args, "show")?, |_, store| Ok(json_of(&views::sending(store.state())))),
            "send" => self.send(arg(args, "show")?, args),
            // What the simulation received: equievents.ch's for a show sending from there.
            "simulation_log" => {
                let show = arg(args, "show")?;
                match self.sends_online(show)? {
                    Some(remote) => self.received_online(&remote),
                    None => Ok(json_of(&self.simulation.received(show))),
                }
            }
            "print" => self.print(arg(args, "show")?, args),
            // The error log, the newest first.
            "error_log" => Ok(Value::Array(self.errors.lock().map(|e| e.iter().rev().cloned().collect()).unwrap_or_default())),
            // Reference data (Rosson's developer mode): a test's or a scoring table's definition.
            "reference_data" => self.with(arg(args, "show")?, |_, store| {
                let state = store.state();
                let programmes: Vec<Value> = state.programmes.values().map(|p| json!({ "id": p.id, "name": p.name, "title": show::master::programme_title(p, "fr") })).collect();
                let tables: Vec<Value> = state.scoring_tables.values().map(|t| json!({ "id": t.id, "code": t.code, "name": t.name })).collect();
                Ok(json!({ "programmes": programmes, "scoring_tables": tables }))
            }),
            "definition" => {
                let id = args["id"].as_u64().ok_or("missing argument id")? as u32;
                let kind = arg(args, "kind")?.to_string();
                self.with(arg(args, "show")?, |_, store| {
                    let state = store.state();
                    let text = match kind.as_str() {
                        "programme" => state.programmes.get(&id).map(|p| p.definition.clone()),
                        "scoring_table" => state.scoring_tables.get(&id).map(|t| t.definition.clone()),
                        other => return Err(format!("no definitions of {other}")),
                    }
                    .ok_or_else(|| format!("unknown {kind} {id}"))?;
                    // Indented, to read and change.
                    let pretty = serde_json::from_str::<Value>(&text).ok().and_then(|v| serde_json::to_string_pretty(&v).ok()).unwrap_or(text);
                    Ok(json!({ "definition": pretty }))
                })
            }
            "set_definition" => {
                let id = args["id"].as_u64().ok_or("missing argument id")? as u32;
                let definition = arg(args, "definition")?.to_string();
                match arg(args, "kind")? {
                    "programme" => self.change(arg(args, "show")?, "set_definition", |state| prepare::set_programme_definition(state, id, &definition)),
                    "scoring_table" => self.change(arg(args, "show")?, "set_definition", |state| prepare::set_scoring_table_definition(state, id, &definition)),
                    other => Err(format!("no definitions of {other}")),
                }
            }
            // Sheets of sticky labels: the Zweckform ones and the office's own.
            "label_sheets" => {
                let builtin = prints::Sheet::builtin().into_iter().map(|s| json!({ "builtin": true, "sheet": s }));
                let own = self.settings().label_sheets.into_iter().map(|s| json!({ "builtin": false, "sheet": s }));
                Ok(Value::Array(builtin.chain(own).collect()))
            }
            "save_label_sheet" => {
                let mut sheet: prints::Sheet = serde_json::from_value(args["sheet"].clone()).map_err(|e| format!("not a sheet: {e}"))?;
                sheet.name = sheet.name.trim().to_string();
                sheet.check()?;
                let mut settings = self.settings();
                if sheet.id.is_empty() || prints::Sheet::builtin().iter().any(|b| b.id == sheet.id) {
                    sheet.id = format!("own-{}", now_millis());
                }
                settings.label_sheets.retain(|s| s.id != sheet.id);
                settings.label_sheets.push(sheet.clone());
                self.save_settings(&settings)?;
                Ok(json!(sheet))
            }
            "remove_label_sheet" => {
                let id = arg(args, "id")?;
                let mut settings = self.settings();
                settings.label_sheets.retain(|s| s.id != id);
                self.save_settings(&settings)?;
                Ok(Value::Null)
            }
            command if sync::COMMANDS.contains(&command) => self.sync_call(command, args),
            other => Err(format!("unknown command {other}")),
        }
    }

    /// Print something (`what`: `start_list` or `ranking` of `class` or `classes` (dressage
    /// `by_judge`: each judge's rank next to their total), `judge_sheets` of
    /// a dressage `class` (one `judge` or all), `combined` ranking `id`, `participants`,
    /// `schedule`), in `language`. The PDF is kept in `<library>/prints/<show>/`; with `inline`, it
    /// also comes back in the answer (base64), for a browser.
    fn print(&self, show: &str, args: &Value) -> Result<Value, String> {
        let lang = prints::Language::of(args["language"].as_str().unwrap_or("fr"));
        let now = now_millis();
        let iso = rfc3339(now);
        let date = rosson_import::swiss_time::date(&iso).unwrap_or_default();
        let time = rosson_import::swiss_time::time_of_day(&iso).map(|t| t.to_string()).unwrap_or_default();
        let printed = format!("{}.{}.{} {time}", date.get(8..10).unwrap_or(""), date.get(5..7).unwrap_or(""), date.get(..4).unwrap_or(""));
        let classes = || -> Result<Vec<ClassId>, String> {
            match args["classes"].as_array() {
                Some(list) => Ok(list.iter().filter_map(|c| c.as_str().map(|c| ClassId(c.into()))).collect()),
                None => Ok(vec![ClassId(arg(args, "class")?.into())]),
            }
        };
        let print = self.with(show, |_, store| {
            let state = store.state();
            match arg(args, "what")? {
                "start_list" => {
                    let kind = match args["variant"].as_str().unwrap_or("full") {
                        "lite" => prints::StartListKind::Lite,
                        "judges" => prints::StartListKind::Judges,
                        _ => prints::StartListKind::Full { pedigree: args["pedigree"].as_bool().unwrap_or(false) },
                    };
                    prints::start_lists_as(state, &classes()?, lang, kind, &printed)
                }
                "ranking" => prints::rankings_with(state, &classes()?, lang, args["by_judge"] == true, &printed),
                "judge_sheets" => {
                    let judge = args["judge"].as_str().map(|j| show::model::AssignmentId(j.into()));
                    prints::judge_sheets(state, &ClassId(arg(args, "class")?.into()), judge.as_ref(), args["memo"].as_bool().unwrap_or(false), lang, &printed)
                }
                "combined" => prints::combined(state, arg(args, "id")?, lang, &printed),
                "control_sheets" => prints::control_sheets(state, &ClassId(arg(args, "class")?.into()), lang, &printed),
                "participants" => Ok(prints::participants(state, lang, &printed)),
                "schedule" => Ok(prints::schedule(state, lang, &printed)),
                "officials" => Ok(prints::officials_plan(state, lang, &printed)),
                "entries" => {
                    let published = args["published"].as_u64().map(PublishedClassId);
                    Ok(prints::entries_detail(state, published.as_ref(), lang, &printed))
                }
                "start_cards" => {
                    // The whole class or one starter; two to an A4 page or one an A5 page.
                    let options = prints::CardOptions { entry: args["entry"].as_u64().map(EntryId), a5: args["paper"] == "a5", ..Default::default() };
                    prints::start_cards_with(state, &ClassId(arg(args, "class")?.into()), &options, lang, &printed)
                }
                "team_cards" => {
                    // Every team or one; an A4 page each or an A5 page.
                    let options = prints::CardOptions { team: args["team"].as_str().map(|t| show::model::TeamId(t.into())), a5: args["paper"] == "a5", ..Default::default() };
                    prints::team_cards_with(state, &ClassId(arg(args, "class")?.into()), &options, lang, &printed)
                }
                "style_sheets" => prints::style_sheets(state, &ClassId(arg(args, "class")?.into()), lang, &printed),
                "letters" => {
                    let place = state.event.as_ref().map(|e| e.place.clone()).unwrap_or_default();
                    let place_date = format!("{place}, {}", prints::long_date(&date, lang, true));
                    Ok(prints::letters(state, args["text"].as_str().unwrap_or(""), args["window"] == "right", &place_date, lang, &printed))
                }
                "labels" => {
                    let what = match arg(args, "kind")? {
                        "addresses" => prints::Labels::Addresses,
                        "judges" => prints::Labels::Judges(ClassId(arg(args, "class")?.into())),
                        "prizes" => prints::Labels::Prizes(ClassId(arg(args, "class")?.into())),
                        other => return Err(format!("no labels called {other}")),
                    };
                    prints::labels(state, &what, self.label_sheet(args["sheet"].as_str().unwrap_or("3475")), args["skip"].as_u64().unwrap_or(0) as usize, lang, &printed)
                }
                other => Err(format!("nothing to print called {other}")),
            }
        })?;
        let pdf = typeset::render(print.template, &print.data)?;
        let dir = self.library.join("prints").join(show);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let name: String = print.name.chars().map(|c| if c.is_alphanumeric() || " .,-_()".contains(c) { c } else { '-' }).collect();
        let path = dir.join(format!("{}.pdf", name.trim()));
        std::fs::write(&path, &pdf.bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut answer = json!({ "path": path, "name": format!("{}.pdf", name.trim()), "pages": pdf.pages });
        if args["inline"] == true {
            answer["pdf"] = json!(base64(&pdf.bytes));
        }
        Ok(answer)
    }

    /// A sheet of labels: the office's own by its id, else a Zweckform one.
    fn label_sheet(&self, id: &str) -> prints::Sheet {
        self.settings().label_sheets.into_iter().find(|s| s.id == id).unwrap_or_else(|| prints::Sheet::of(id))
    }

    /// Conflicts looked at on this computer (those no longer in the show are dropped).
    fn acknowledge(&self, show: &str, ids: &[String]) -> Result<(), String> {
        self.with(show, |_, store| {
            let mut seen = conflicts::seen_ids(store.meta(conflicts::META_SEEN).map_err(|e| e.0)?);
            seen.extend(ids.iter().cloned());
            let known: std::collections::BTreeSet<&String> = store.state().conflicts.iter().map(|c| &c.id).collect();
            let kept: Vec<&String> = seen.iter().filter(|i| known.contains(i)).collect();
            store.set_meta(conflicts::META_SEEN, Some(&serde_json::to_string(&kept).map_err(|e| e.to_string())?)).map_err(|e| e.0)
        })
    }

    /// Who makes the changes here: the account signed in (the name others see on the show),
    /// else this computer's user.
    fn author(&self) -> String {
        self.settings().account.filter(|a| a.server == self.server_url() && !a.name.trim().is_empty()).map_or_else(|| self.operator.clone(), |a| a.name)
    }

    fn settings(&self) -> Settings {
        std::fs::read(self.library.join("settings.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn save_settings(&self, settings: &Settings) -> Result<(), String> {
        let path = self.library.join("settings.json");
        std::fs::write(&path, serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// A training show (the practice event, Swiss Equestrian's demo event): never synced.
    pub(crate) fn is_training(&self, show: &str) -> Result<bool, String> {
        self.with(show, |_, store| Ok(training::is_training(store.first_author().map_err(|e| e.0)?.as_deref())))
    }

    fn event_id(&self, show: &str) -> Result<u64, String> {
        self.with(show, |_, store| store.state().event.as_ref().map(|e| e.id.0).ok_or_else(|| "the show has no event from Swiss Equestrian".to_string()))
    }

    /// Where a show's sends go (equievents.ch's choice for a show there, `null` when it does not
    /// answer; else the simulation here), and whether its event's access key is known.
    fn connection(&self, show: &str) -> Result<Value, String> {
        let event = self.event_id(show)?;
        let training = self.is_training(show)?;
        let online = self.sends_online(show)?.is_some();
        let server = if online { self.server_sends() } else { Some("simulation".to_string()) };
        let key = if training { Some(training::SIMULATION_KEY.to_string()) } else { self.secrets.get(&connection::access_key_name(event))? };
        let rehearsal = self.with(show, |_, store| Ok(store.first_author().map_err(|e| e.0)?.as_deref() == Some(REHEARSAL)))?;
        Ok(json!({
            "training": training,
            "rehearsal": rehearsal,
            "online": online,
            "server": server,
            "has_key": key.is_some(),
            // Enough to recognise it, not to use it.
            "key_end": key.map(|k| k.chars().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<String>()),
        }))
    }

    /// The keys of a request to the simulation here (its event's access key, today's
    /// rosson_key).
    fn keys(&self, show: &str) -> Result<se_api::Keys, String> {
        if self.is_training(show)? {
            self.keys_with(training::SIMULATION_KEY.to_string())
        } else {
            self.event_keys(show)
        }
    }

    /// The keys with the show's event's access key (kept from its Rosson backup, or entered).
    fn event_keys(&self, show: &str) -> Result<se_api::Keys, String> {
        let event = self.event_id(show)?;
        let access_key = self.secrets.get(&connection::access_key_name(event))?.ok_or("enter the event's access key first (given by Swiss Equestrian)")?;
        self.keys_with(access_key)
    }

    /// An access key with today's rosson_key and Rosson's version.
    fn keys_with(&self, access_key: String) -> Result<se_api::Keys, String> {
        let today = rosson_import::swiss_time::date(&rfc3339(now_millis())).ok_or("today's date")?;
        let ymd: Vec<u32> = today.split('-').filter_map(|p| p.parse().ok()).collect();
        let rosson_key = se_api::rosson_key(ymd[0], ymd[1], ymd[2]);
        Ok(se_api::Keys { access_key, rosson_key, rosson_version: se_api::version::current(None, self.settings().rosson_version.as_deref()) })
    }

    /// Tests: Swiss Equestrian's riders and horses from the simulation (the library's own), so
    /// that no test reaches Swiss Equestrian.
    pub fn simulate_registry(&self) {
        self.simulated_registry.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// What the simulation answers to a registry request: the riders (with their contact data,
    /// on this computer already) and horses of the library's shows.
    fn simulated_read(&self, endpoint: &str) -> Option<Value> {
        let list = match endpoint {
            "getPersonen" => "personen",
            "getPferde" => "pferde",
            _ => return None,
        };
        let ids: Vec<String> = self.show_ids().ok()?;
        let mut records: std::collections::BTreeMap<u64, Value> = std::collections::BTreeMap::new();
        for id in ids {
            let _ = self.with(&id, |_, store| {
                let state = store.state();
                if list == "personen" {
                    for p in state.persons.values() {
                        let c = state.contacts.get(&p.id).cloned().unwrap_or_default();
                        records.entry(p.id.0).or_insert_with(|| {
                            json!({ "id": p.id.0, "vorname": p.first_name, "nachname": p.last_name, "ort": p.town, "land": p.nationality, "vereinsname": p.club, "lizenzen": p.licences,
                                    "nachwuchsreiter": p.youth, "anrede": c.salutation, "strasse": c.street, "postleitzahl": c.postcode, "email": c.email,
                                    "telefon_privat": c.phone_private, "telefon_mobil": c.phone_mobile, "sprache": c.language, "geboren_am": c.birth_date })
                        });
                    }
                } else {
                    for h in state.horses.values().filter(|h| h.id.0 != se_api::load::PLACEHOLDER_HORSE) {
                        let record = if h.record.is_object() { h.record.clone() } else { json!({ "id": h.id.0, "name": h.name, "signalement_label": h.signalement, "besitzer": h.owner, "zuechter": h.breeder }) };
                        records.entry(h.id.0).or_insert(record);
                    }
                }
                Ok(())
            });
        }
        Some(json!({ "data": { list: records.into_values().collect::<Vec<_>>() } }))
    }

    /// Swiss Equestrian's riders and horses (`getPersonen`, `getPferde`, Rosson's `alle_personen`
    /// and `pferde`): equievents.ch downloads them with the show's key and keeps them for the
    /// account, then this computer gets the account's copy (`Office::registry_via_server`); the
    /// app never reaches Swiss Equestrian. Tests (`simulate_registry`): the simulation here
    /// answers with the library's own riders and horses, with the show's access key, as Rosson
    /// asks for them.
    fn download_registry(&self, show: &str) -> Result<Value, String> {
        if !self.simulated_registry.load(std::sync::atomic::Ordering::Relaxed) {
            return self.registry_via_server(show);
        }
        let keys = self.event_keys(show)?;
        let fetch = |request: se_api::Request| -> Result<Value, String> {
            connection::block_on(se_api::http::run(
                connection::SIMULATION,
                &request,
                se_api::http::Limits::default(),
                |http| std::future::ready(Ok(self.simulation.answer(show, &http, &keys.rosson_key, now_millis(), &|endpoint| self.simulated_read(endpoint)))),
                |_| std::future::ready(()),
            ))
            .map_err(|e| e.to_string())
        };
        let persons = se_api::load::read_registry(&fetch(se_api::load::get_personen(&keys))?, "personen")?;
        let horses = se_api::load::read_registry(&fetch(se_api::load::get_pferde(&keys))?, "pferde")?;
        Ok(json_of(&self.registry.store(Some(persons), Some(horses), now_millis(), true, None)?))
    }

    /// The rider the office picked (`{"person": id}`): the registry's record when it has them
    /// (the show's own record wins when the show has them too), else one of the show.
    fn rider_pick(&self, v: &Value) -> Result<entries::RiderPick, String> {
        let id = v["person"].as_u64().ok_or("a rider of the show or of Swiss Equestrian's registry")?;
        Ok(match self.registry.person(id) {
            Some(record) => entries::RiderPick::Registry(Box::new(se_api::load::person_of(id, &record))),
            None => entries::RiderPick::Known(show::model::PersonId(id)),
        })
    }

    /// The horse the office picked: `{"horse": id}` (the show's or the registry's) or
    /// `{"guest": name}`.
    fn horse_pick(&self, v: &Value) -> Result<entries::HorsePick, String> {
        if let Some(name) = v["guest"].as_str() {
            return Ok(entries::HorsePick::Guest(name.to_string()));
        }
        let id = v["horse"].as_u64().ok_or("a horse of the show or of Swiss Equestrian's registry, or a guest horse")?;
        Ok(match self.registry.horse(id) {
            Some(record) => entries::HorsePick::Registry(Box::new(se_api::load::horse_of(id, &record))),
            None => entries::HorsePick::Known(HorseId(id)),
        })
    }

    /// Send something to Swiss Equestrian (`what`: `results` of a class, `start_lists` or
    /// `withdraw_start_lists` of a published class, `event`, `combined`); what it answered is
    /// recorded. A show of equievents.ch sends from there (only the server talks to Swiss
    /// Equestrian); a show only on this computer, a rehearsal or a training show, to the
    /// simulation here.
    fn send(&self, show: &str, args: &Value) -> Result<Value, String> {
        use se_api::{actions, http::Limits};
        if let Some(remote) = self.sends_online(show)? {
            return self.send_online(show, &remote, args);
        }
        let keys = self.keys(show)?;
        let rosson_key = keys.rosson_key.clone();
        let what = arg(args, "what")?.to_string();
        let language = match args["language"].as_str() {
            Some("fr") => se_api::startlists::Language::Fr,
            Some("it") => se_api::startlists::Language::It,
            _ => se_api::startlists::Language::De,
        };
        let steps = self.with(show, |_, store| {
            let state = store.state();
            let class = || arg(args, "class").map(|c| ClassId(c.into()));
            let published = || args["published"].as_u64().map(PublishedClassId).ok_or("missing argument published");
            match what.as_str() {
                "results" => actions::results(state, &class()?, &keys),
                "start_lists" => actions::start_lists(state, &published()?, &keys, language),
                "withdraw_start_lists" => Ok(actions::withdraw_start_lists(&published()?, &keys)),
                "event" => Ok(actions::event(state, &keys)),
                "combined" => actions::combined(state, arg(args, "id")?, &keys),
                other => Err(format!("nothing to send called {other}")),
            }
        })?;
        // The show is not held while the requests go out.
        let outcome = connection::block_on(actions::perform(
            steps,
            connection::SIMULATION,
            Limits::default(),
            |request| std::future::ready(Ok(self.simulation.answer(show, &request, &rosson_key, now_millis(), &|endpoint| self.simulated_read(endpoint)))),
            |_| std::future::ready(()),
        ));
        if let Some(e) = outcome.failed {
            return Err(e.to_string());
        }
        self.with(show, |_, store| {
            let ops = self.operations(outcome.changes, &self.author(), store.state().last.as_ref());
            store.append(&ops).map_err(|e| e.0)
        })?;
        Ok(json!({ "sent": what, "server": connection::Server::Simulation, "warnings": outcome.warnings }))
    }

    fn path(&self, id: &str) -> PathBuf {
        self.library.join(format!("{id}.{SHOW_EXTENSION}"))
    }

    /// Run `f` on an open show.
    fn with<T>(&self, id: &str, f: impl FnOnce(&str, &mut Store) -> Result<T, String>) -> Result<T, String> {
        if id.contains(['/', '\\']) || id.starts_with('.') {
            return Err(format!("not a show: {id}"));
        }
        let mut open = self.open.lock().map_err(|_| "the library is busy".to_string())?;
        if !open.contains_key(id) {
            let path = self.path(id);
            if !path.is_file() {
                return Err(format!("no show {id} in the library"));
            }
            open.insert(id.into(), Store::open(&path).map_err(|e| e.0)?);
        }
        f(id, open.get_mut(id).unwrap())
    }

    /// The ids of the shows in the library.
    fn show_ids(&self) -> Result<Vec<String>, String> {
        let mut ids: Vec<String> = std::fs::read_dir(&self.library)
            .map_err(|e| e.to_string())?
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                (p.extension().and_then(|x| x.to_str()) == Some(SHOW_EXTENSION)).then(|| p.file_stem()?.to_str().map(str::to_string)).flatten()
            })
            .collect();
        ids.sort();
        Ok(ids)
    }

    /// The shows in the library, as summaries (`views::ShowSummary`). Quick: a show not open is
    /// not rebuilt from its log; its summary is kept in its file with the number of operations
    /// it was made at, and holds while that number does (operations are only ever added).
    pub fn library(&self) -> Result<Vec<Value>, String> {
        const KEPT: &str = "library_summary_2";
        let summarise = |id: &str, store: &mut Store| -> Result<Value, String> {
            let first = store.first_author().map_err(|e| e.0)?;
            let (rehearsal, training) = (first.as_deref() == Some(REHEARSAL), training::is_training(first.as_deref()));
            let summary = json_of(&views::summary(id, store.state(), rehearsal, training));
            let kept = json!({ "operations": store.operation_count().map_err(|e| e.0)?, "summary": summary }).to_string();
            if store.meta(KEPT).map_err(|e| e.0)?.as_deref() != Some(kept.as_str()) {
                store.set_meta(KEPT, Some(&kept)).map_err(|e| e.0)?;
            }
            Ok(summary)
        };
        let server = self.server_url();
        let mut out = Vec::new();
        for id in self.show_ids()? {
            let open = self.open.lock().map_err(|_| "the library is busy".to_string())?.contains_key(&id);
            let peek = show::store::Peek::open(&self.path(&id)).map_err(|e| e.0)?;
            // Not open: its kept summary, if it still holds.
            let kept = if open { None } else { peek.meta(KEPT).and_then(|k| serde_json::from_str::<Value>(&k).ok()) };
            let mut summary = match (kept, peek.operation_count()) {
                (Some(kept), Ok(count)) if kept["operations"].as_u64() == Some(count as u64) => {
                    let mut summary = kept["summary"].clone();
                    summary["id"] = json!(id);
                    summary
                }
                _ => self.with(&id, summarise)?,
            };
            // On equievents.ch (one's role there) or here only; in the archives or not.
            let online = peek.meta(sync::META_SERVER).as_deref() == Some(server.as_str()) && peek.meta(sync::META_REMOTE).is_some();
            summary["online"] = json!(online);
            summary["role"] = json!(if online { peek.meta(sync::META_ROLE) } else { None });
            summary["archived"] = json!(peek.meta(sync::META_ARCHIVED).as_deref() == Some("1"));
            out.push(summary);
        }
        Ok(out)
    }

    /// A free id for a new show, from the event number.
    fn new_id(&self, base: &str) -> String {
        let base: String = base.chars().map(|c| if c.is_alphanumeric() || matches!(c, '.' | '-' | ' ') { c } else { '_' }).collect();
        (1..).map(|n| if n == 1 { base.clone() } else { format!("{base} ({n})") }).find(|id| !self.path(id).exists()).unwrap()
    }

    /// Create a show from operations.
    fn create(&self, base: &str, ops: &[Operation]) -> Result<String, String> {
        let id = self.new_id(base);
        let mut store = Store::open(&self.path(&id)).map_err(|e| e.0)?;
        store.append(ops).map_err(|e| e.0)?;
        if let Some((op, why)) = store.ignored().first() {
            let _ = std::fs::remove_file(self.path(&id));
            return Err(format!("the import is inconsistent (operation {op}: {why})"));
        }
        self.open.lock().map_err(|_| "the library is busy".to_string())?.insert(id.clone(), store);
        Ok(id)
    }

    /// Import an event from a Rosson export: as it ended, or as Swiss Equestrian delivered it (to
    /// rehearse the show; past events cannot be fetched from Swiss Equestrian).
    pub fn import_rosson(&self, path: &Path, event: &str, delivered: bool) -> Result<String, String> {
        let export = read_rosson(path)?;
        let snapshot = export.event(event).map_err(|e| e.0)?;
        let number = export.events().into_iter().find(|e| e.id == event).map(|e| e.number).unwrap_or_default();
        if !delivered {
            let ops = rosson_import::ops::to_operations(&snapshot).map_err(|e| e.0)?;
            let show = self.create(&number, &ops)?;
            // The event's access key, when the file has it: the show goes on with it.
            if let (Some(key), Ok(event_id)) = (export.access_key(event), event.parse::<u64>()) {
                self.secrets.set(&connection::access_key_name(event_id), Some(&key))?;
            }
            return Ok(show);
        }
        let now = now_millis();
        let master = se_api::load::read_master_data(&rosson_import::delivered::master_data(&snapshot), now)?;
        let officials = se_api::load::read_officials(&rosson_import::delivered::officials(&snapshot))?;
        let event_id: u64 = event.parse().unwrap_or(0);
        let changes = se_api::load::read_event(&rosson_import::delivered::event(&snapshot), &master, &officials, now, &mut || se_api::load::assignment_id(event_id, now, self.random()))?;
        let ops = self.operations(changes, REHEARSAL, None);
        self.create(&format!("{number} rehearsal"), &ops)
    }

    /// The practice event as a new show (« Concours d'exercice »), from today, in `language`.
    pub fn create_practice(&self, language: &str) -> Result<String, String> {
        let today = rosson_import::swiss_time::date(&rfc3339(now_millis())).ok_or("today's date")?;
        let (master, officials, event) = training::practice(&today, language)?;
        self.training_show(training::PRACTICE, &master, &officials, &event)
    }

    /// Swiss Equestrian's demo event (Rosson's « Manifestation test »; `driving`: its driving
    /// one), downloaded by equievents.ch (`Office::demo_via_server`: the app never reaches Swiss
    /// Equestrian), as a new show on this computer. Its answers are kept in the library's
    /// `swiss-equestrian` folder, to look at.
    pub fn load_demo(&self, driving: bool) -> Result<String, String> {
        let answers = self.demo_via_server(driving)?;
        let kept = self.library.join("swiss-equestrian");
        let key = connection::DEMO_KEYS[usize::from(driving)];
        for name in ["getStammdaten", "getOffizielle", "getVeranstaltung"] {
            if std::fs::create_dir_all(&kept).is_ok() {
                let _ = std::fs::write(kept.join(format!("{key}-{name}.json")), serde_json::to_vec_pretty(&answers[name]).unwrap_or_default());
            }
        }
        self.training_show(training::DEMO, &answers["getStammdaten"], &answers["getOffizielle"], &answers["getVeranstaltung"])
    }

    /// A training show from Swiss Equestrian's three answers, read as a real event is.
    fn training_show(&self, author: &str, master: &Value, officials: &Value, event: &Value) -> Result<String, String> {
        let now = now_millis();
        let master = se_api::load::read_master_data(master, now)?;
        let officials = se_api::load::read_officials(officials)?;
        let event_id = se_api::load::number(&event["data"]["veranstaltung"]["id"]).unwrap_or(0);
        let number = event["data"]["veranstaltung"]["nr"].as_str().unwrap_or("training").to_string();
        let changes = se_api::load::read_event(event, &master, &officials, now, &mut || se_api::load::assignment_id(event_id, now, self.random()))?;
        let ops = self.operations(changes, author, None);
        self.create(&number, &ops)
    }

    /// Import an EquiEvents show file as a new show (or into the show it is a copy of).
    pub fn import_show(&self, path: &Path) -> Result<String, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let (manifest, ops) = show::export::read(&bytes).map_err(|e| e.0)?;
        let number = manifest.event.map(|e| e.number).unwrap_or_else(|| "show".into());
        // A copy of a show in the library gets the operations it lacks.
        for summary in self.library()? {
            let same = summary["event"]["number"].as_str() == Some(number.as_str());
            let id = summary["id"].as_str().unwrap_or_default().to_string();
            if same {
                let found = self.with(&id, |_, store| Ok(store.log().map_err(|e| e.0)?.first().map(|o| o.id.clone()) == ops.first().map(|o| o.id.clone())))?;
                if found {
                    self.with(&id, |_, store| show::export::import_into(store, &bytes).map_err(|e| e.0))?;
                    return Ok(id);
                }
            }
        }
        self.create(&number, &ops)
    }

    pub fn export_show(&self, id: &str, path: &Path, privacy: show::export::Privacy) -> Result<(), String> {
        let bytes = self.with(id, |_, store| {
            let log = store.log().map_err(|e| e.0)?;
            show::export::write(&log, privacy, env!("CARGO_PKG_VERSION"), &rfc3339(now_millis())).map_err(|e| e.0)
        })?;
        std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Move a show out of the library (into its `deleted` folder: nothing is lost by mistake).
    pub fn delete_show(&self, id: &str) -> Result<(), String> {
        self.open.lock().map_err(|_| "the library is busy".to_string())?.remove(id);
        let trash = self.library.join("deleted");
        std::fs::create_dir_all(&trash).map_err(|e| e.to_string())?;
        std::fs::rename(self.path(id), trash.join(format!("{id} {}.{SHOW_EXTENSION}", now_millis()))).map_err(|e| e.to_string())
    }

    /// Make changes to a show: computed from its state by `f`, appended as operations, their undo
    /// kept. Answers what can be undone next, by command name (the UI says it in its language).
    fn change(&self, id: &str, what: &str, f: impl FnOnce(&show::State) -> Result<Vec<show::oplog::Change>, String>) -> Result<Value, String> {
        let author = self.author();
        let undo = self.with(id, |_, store| {
            let mut changes = f(store.state())?;
            if changes.is_empty() {
                return Ok(None);
            }
            // What the author saw (R7, R11): applied after others' changes, only theirs counts.
            store.state().fill_seen(&mut changes);
            let mut undo = prepare::inverse(store.state(), &changes);
            let ops = self.operations(changes, &author, store.state().last.as_ref());
            store.append(&ops).map_err(|e| e.0)?;
            // Undone later, after others' changes came: only this change is taken back.
            store.state().fill_seen(&mut undo);
            // A change the show refused (e.g. a final class) is an error for the office.
            let ignored: Vec<String> = store.ignored().iter().filter(|(op, _)| ops.iter().any(|o| &o.id == op)).map(|(_, why)| why.clone()).collect();
            if !ignored.is_empty() {
                return Err(ignored.join("; "));
            }
            Ok(Some(undo))
        })?;
        // Nothing changed: nothing to undo either.
        let Some(undo) = undo else {
            let top = self.undo.lock().unwrap_or_else(|e| e.into_inner()).get(id).and_then(|s| s.last()).map(|(w, _)| w.clone());
            return Ok(json!({ "undo": top, "unchanged": true }));
        };
        push(&self.undo, id, what.to_string(), undo);
        // A new change: what was undone before it cannot come back any more.
        self.redo.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
        Ok(json!({ "undo": what }))
    }

    /// Undo the office's last change to a show (as new operations: the log keeps both). What
    /// was undone can be redone until the next change.
    fn undo(&self, id: &str) -> Result<Value, String> {
        let (what, next) = self.step(id, &self.undo, &self.redo, "undo")?;
        Ok(json!({ "undone": what, "undo": next, "redo": what }))
    }

    /// Redo what was last undone (it can be undone again).
    fn redo(&self, id: &str) -> Result<Value, String> {
        let (what, next) = self.step(id, &self.redo, &self.undo, "redo")?;
        Ok(json!({ "redone": what, "undo": what, "redo": next }))
    }

    /// Apply the changes on top of `from`, keeping their inverse on `to`: undo and redo. Answers
    /// what was applied and what is next on `from`.
    fn step(&self, id: &str, from: &Mutex<HashMap<String, UndoStack>>, to: &Mutex<HashMap<String, UndoStack>>, how: &str) -> Result<(String, Option<String>), String> {
        let last = from.lock().unwrap_or_else(|e| e.into_inner()).get_mut(id).and_then(Vec::pop);
        let Some((what, changes)) = last else { return Err(format!("nothing to {how}")) };
        let author = format!("{} ({how})", self.author());
        let back = self.with(id, |_, store| {
            let mut back = prepare::inverse(store.state(), &changes);
            let ops = self.operations(changes.clone(), &author, store.state().last.as_ref());
            store.append(&ops).map_err(|e| e.0)?;
            store.state().fill_seen(&mut back);
            Ok(back)
        });
        let back = match back {
            Ok(back) => back,
            Err(e) => {
                // Not applied: it stays where it was.
                push(from, id, what, changes);
                return Err(e);
            }
        };
        push(to, id, what.clone(), back);
        let next = from.lock().unwrap_or_else(|e| e.into_inner()).get(id).and_then(|s| s.last()).map(|(w, _)| w.clone());
        Ok((what, next))
    }

    /// Changes made here, as operations of this device: stamped after everything the show knows
    /// (`after`), never twice the same (two changes in one millisecond keep their own ids).
    fn operations(&self, changes: Vec<show::oplog::Change>, author: &str, after: Option<&show::oplog::Stamp>) -> Vec<Operation> {
        let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(stamp) = after {
            clock.observe(stamp);
        }
        let now = now_millis();
        changes
            .into_iter()
            .map(|change| {
                let stamp = clock.tick(now);
                Operation { id: format!("{}:{}:{}", self.device, stamp.millis, stamp.counter), stamp, author: author.into(), change }
            })
            .collect()
    }

    /// A random number (xorshift; for Rosson-style assignment ids, not for secrets).
    fn random(&self) -> u64 {
        let mut r = self.random.lock().unwrap_or_else(|e| e.into_inner());
        *r ^= *r << 13;
        *r ^= *r >> 7;
        *r ^= *r << 17;
        *r % 36u64.pow(11)
    }
}

fn read_rosson(path: &Path) -> Result<rosson_import::export::RossonExport, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    rosson_import::export::read(&bytes).map_err(|e| e.0)
}

/// `2026-11-13T12:00:00Z`
fn rfc3339(millis: u64) -> String {
    let secs = millis / 1000;
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // Civil date from days since 1970 (H. Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

/// Standard Base64 (a PDF sent to a browser).
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() { ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// A number from the UI: a JSON number or text (`6,5` too); `null` or empty text: none.
fn number(v: &Value) -> Result<Option<rust_decimal::Decimal>, String> {
    let text = match v {
        Value::Null => return Ok(None),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.trim().replace(',', "."),
        _ => return Err("not a number".into()),
    };
    if text.is_empty() {
        return Ok(None);
    }
    scoring::number::parse(&text).map_err(|_| format!("{text} is not a number"))?.map(Some).ok_or_else(|| format!("{text} is not a number"))
}

fn json_of<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

#[cfg(test)]
#[path = "../tests/common/mod.rs"]
mod test_common;

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_common as common;
    use show::model::{Contact, Entry, EntryStatus, PersonId};
    use show::oplog::Change;

    /// A rider's address is Swiss Equestrian's: the office reads it, never sets it (an address
    /// set by an earlier version, in a show's log, still stays when the rider is delivered again).
    #[test]
    fn addresses_are_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let office = Office::new(&dir.path().join("library"), "test").unwrap();
        let file = common::rosson_file(dir.path(), "2026.8352");
        let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
        let show = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
        let person = office.with(&show, |_, store| Ok(store.state().entries.values().next().unwrap().rider.0)).unwrap();
        let read = |office: &Office| office.call("address", &json!({ "show": show, "person": person })).unwrap();
        assert!(office.call("set_address", &json!({ "show": show, "person": person, "street": "Rue du Haras 3" })).unwrap_err().contains("unknown command"));
        // An older log's address, then Swiss Equestrian's delivery: the address stays.
        office
            .change(&show, "test", |_| Ok(vec![Change::AddressSet { person: PersonId(person), town: "Avenches".into(), contact: Contact { person: PersonId(person), street: "Rue du Haras 3".into(), ..Default::default() } }]))
            .unwrap();
        office.change(&show, "test", |_| Ok(vec![Change::PrivateDataDelivered { contacts: vec![Contact { person: PersonId(person), street: "Elsewhere 1".into(), ..Default::default() }], notes: vec![] }])).unwrap();
        assert_eq!((read(&office)["street"].as_str(), read(&office)["town"].as_str()), (Some("Rue du Haras 3"), Some("Avenches")));
    }

    /// An entry Swiss Equestrian delivers after its class was set up is on no start list
    /// (Rosson's "Ohne Prüfung") until the office places it.
    #[test]
    fn an_entry_on_no_start_list() {
        let dir = tempfile::tempdir().unwrap();
        let office = Office::new(&dir.path().join("library"), "test").unwrap();
        let file = common::rosson_file(dir.path(), "2026.8352");
        let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
        let show = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
        let published = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == "2").unwrap()["id"].as_u64().unwrap();
        let class = office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string();
        office.call("number", &json!({ "show": show, "class": class, "first": 1 })).unwrap();
        let list = |office: &Office| office.call("start_list", &json!({ "show": show, "class": class })).unwrap();
        assert_eq!(list(&office)["unplaced"], json!([]));

        // A new entry delivered: like another of the class, with a new id.
        let late = EntryId(999_001);
        office
            .change(&show, "test", |state| {
                let model = state.entries.values().find(|e| e.published.0 == published && e.status == EntryStatus::Entered).unwrap();
                Ok(vec![Change::EntryDelivered { entry: Entry { id: late.clone(), ..model.clone() } }])
            })
            .unwrap();
        let view = list(&office);
        assert_eq!(view["unplaced"].as_array().unwrap().iter().map(|u| u["entry"].as_u64().unwrap()).collect::<Vec<_>>(), vec![late.0]);
        assert_eq!(view["class"]["unplaced"], 1);

        // Placed: at the end, with the next number.
        let highest = view["lines"].as_array().unwrap().iter().filter_map(|l| l["start_number"].as_u64()).max().unwrap();
        office.call("place_entry", &json!({ "show": show, "entry": late.0, "class": class })).unwrap();
        let view = list(&office);
        let last = view["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").last().unwrap().clone();
        assert_eq!((last["entry"].as_u64(), last["start_number"].as_u64()), (Some(late.0), Some(highest + 1)));
        assert_eq!(view["unplaced"], json!([]));
        assert!(office.call("place_entry", &json!({ "show": show, "entry": late.0, "class": class })).is_err());

        // Undone: on no list again.
        office.call("undo", &json!({ "show": show })).unwrap();
        assert_eq!(list(&office)["unplaced"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn dates() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_794_571_200_000), "2026-11-13T12:00:00Z");
    }
}
