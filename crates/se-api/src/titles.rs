//! The titles of Swiss Equestrian's officials, in German, French and Italian, as its public
//! search of officials shows them (info.swiss-equestrian.ch, Personensuche › Offizielle; the list
//! is `mitgliedschaften/offizielle.json?rollen=true`, read on 2026-11-15). The national list of
//! officials (`getOffizielle`) names each official's roles by these German titles
//! (`rolle_bezeichnung_de`) and gives the French and Italian beside them; this list says them in
//! every language for a show that kept only the German, and gives Swiss Equestrian's order
//! (dressage, jumping, eventing, driving…, trainers last).
//!
//! Public, no personal data. To refresh it, read that list again and write the rows the same way.

/// One title of an official.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Title {
    pub id: u32,
    pub code: &'static str,
    pub de: &'static str,
    pub fr: &'static str,
    pub it: &'static str,
    /// Swiss Equestrian's order (`sort_index`); none for a few recent titles.
    pub order: Option<u32>,
    /// An FEI title (`ist_fei_funktion`).
    pub fei: bool,
}

/// The title with this German name (as the national list of officials gives it; spaces do not
/// count).
pub fn title(de: &str) -> Option<&'static Title> {
    let wanted: Vec<&str> = de.split_whitespace().collect();
    TITLES.iter().find(|t| t.de.split_whitespace().eq(wanted.iter().copied()))
}

/// Every title, by id.
pub const TITLES: &[Title] = &[
    Title { id: 1, code: "ABRK", de: "Ausbilder Brevet klassisch", fr: "Instructeur brevet classique", it: "Istruttore di Brevetto monta classica", order: Some(11060), fei: false },
    Title { id: 3, code: "ABS", de: "Ausbilder Behindertensport", fr: "Instructeur sport handicap", it: "Istruttore attività sportiva con handicap", order: Some(11063), fei: false },
    Title { id: 7, code: "ANW", de: "Anwärter", fr: "Aspirant", it: "Aspirante", order: Some(3022), fei: false },
    Title { id: 18, code: "J+S COACH", de: "J+S COACH", fr: "J+S COACH", it: "G+S COACH", order: Some(1063), fei: false },
    Title { id: 24, code: "DCSTFEI", de: "Steward FEI Dressage", fr: "Steward FEI Dressage", it: "Steward FEI Dressage", order: Some(13040), fei: true },
    Title { id: 27, code: "DGSTFEI", de: "General Steward FEI Dressage", fr: "General Steward FEI Dressage", it: "General Steward FEI Dressage", order: Some(13020), fei: true },
    Title { id: 30, code: "DICSTFEI", de: "Steward FEI Endurance", fr: "Steward FEI Endurance", it: "Steward FEI Endurance", order: Some(13170), fei: true },
    Title { id: 34, code: "DIRI", de: "Endurancerichter", fr: "Juge de raids d'endurance", it: "Giudice di endurance", order: Some(10040), fei: false },
    Title { id: 37, code: "DIRIFEII", de: "Endurancerichter International FEI", fr: "Juge international FEI raids d'endurance", it: "Giudice internazionele FEI endurance", order: Some(10020), fei: true },
    Title { id: 41, code: "DRIFEII", de: "Dressurrichter International FEI", fr: "Juge international FEI dressage", it: "Giudice internazionale FEI dressage", order: Some(5030), fei: true },
    Title { id: 45, code: "DRIL", de: "Dressurrichter L", fr: "Juge de dressage L", it: "Giudice di dressage L", order: Some(5090), fei: false },
    Title { id: 46, code: "DRILA", de: "Dressurrichter Anwärter L", fr: "Aspirant-juge de dressage L", it: "Aspirante giudice di dressage L", order: Some(5092), fei: false },
    Title { id: 47, code: "DRIM", de: "Dressurrichter M", fr: "Juge de dressage M", it: "Giudice di dressage M", order: Some(5078), fei: false },
    Title { id: 48, code: "DRIM*", de: "Dressurrichter M*", fr: "Juge de dressage M*", it: "Giudice di dressage M*", order: Some(5080), fei: false },
    Title { id: 49, code: "DRIS*", de: "Dressurrichter S*", fr: "Juge de dressage S*", it: "Giudice di dressage S*", order: Some(5075), fei: false },
    Title { id: 50, code: "DRISG", de: "Dressurrichter S grosse Tour", fr: "Juge de dressage S Grand tour", it: "Giudice di dressage S Giro grande", order: Some(5060), fei: false },
    Title { id: 51, code: "DRISK", de: "Dressurrichter S kleine Tour", fr: "Juge de dressage S Petit tour", it: "Giudice di dressage S Giro piccolo", order: Some(5070), fei: false },
    Title { id: 52, code: "DTD", de: "Dressur Turnier-TD", fr: "DT du concours", it: "DT del concorso", order: Some(5050), fei: false },
    Title { id: 56, code: "J+S EXP JUG", de: "J+S Experte/in Jugendliche Pferdesport", fr: "J+S Expert/e Sport des Jeunes sports équestres", it: "G+S Esperto/a per Giovani degli sport equestri", order: Some(2053), fei: false },
    Title { id: 58, code: "FCSTFEI", de: "Steward FEI Driving", fr: "Steward FEI Driving", it: "Steward FEI Driving", order: Some(13140), fei: true },
    Title { id: 68, code: "FPBFEII", de: "Fahren Parcoursbauer International FEI", fr: "Constructeur parcours international FEI attelage", it: "Costruttore di percorsi internazionale FEI attacchi", order: Some(12150), fei: true },
    Title { id: 71, code: "FPBRN", de: "Fahren Parcoursbauer National", fr: "Constructeur de parcours d'attelage national", it: "Costruttore di percorsi attacchi Nazionale", order: Some(12168), fei: false },
    Title { id: 75, code: "FRI", de: "Fahrrichter", fr: "Juge d'attelage", it: "Giudice di attacchi", order: Some(8040), fei: false },
    Title { id: 76, code: "FRIA", de: "Fahrrichter Anwärter", fr: "Aspirant-juge d'attelage", it: "Aspirante giudice di attacchi", order: Some(8050), fei: false },
    Title { id: 79, code: "FRIFEIO", de: "Fahrrichter Offiziell FEI", fr: "Juge officiel FEI attelage", it: "Giudice ufficiale FEI attacchi", order: Some(8010), fei: true },
    Title { id: 83, code: "GYMPB", de: "Gymkhana Parcoursbauer", fr: "Gymkhana constructeur de parcours", it: "Costruttore di percorsi gimkana", order: Some(10060), fei: false },
    Title { id: 84, code: "GYMR", de: "Gymkhana Richter", fr: "Gymkhana juge", it: "Giudice di gimkana", order: Some(10050), fei: false },
    Title { id: 89, code: "J+S LEIT JUG", de: "J+S Leiter/in Jugendsport Pferdesport", fr: "J+S Moniteur/trice Sport des Jeunes sports équestres", it: "G+S Monitore/trice per Giovani degli sport equestri", order: Some(1076), fei: false },
    Title { id: 90, code: "J+S LEIT K", de: "J+S Leiter/in Kids", fr: "J+S Moniteur/trice Kids", it: "G+S Monitore/trice Kids", order: Some(1077), fei: false },
    Title { id: 92, code: "LIZLEIT", de: "Lizenzprüfungsleiter", fr: "Chef d'examen de licence", it: "Direttore dell'esame di licenza", order: Some(11010), fei: false },
    Title { id: 93, code: "LIZRAR", de: "Lizenzrichter Anwärter Reiten", fr: "Aspirant-juge de licence équitation", it: "Aspirante giudice di licenza equitazione", order: Some(11030), fei: false },
    Title { id: 94, code: "LIZRIREI", de: "Lizenzrichter Reiten", fr: "Juge de licence équitation", it: "Giudice di Licenza Equitazione", order: Some(11020), fei: false },
    Title { id: 95, code: "LIZRIFA", de: "Lizenzrichter Fahren", fr: "Juge de licence attelage", it: "Giudice di Licenza attacchi", order: Some(11021), fei: false },
    Title { id: 97, code: "MCSTFEI", de: "Steward FEI Eventing", fr: "Steward FEI Eventing", it: "Steward FEI Eventing", order: Some(13110), fei: true },
    Title { id: 102, code: "MJP", de: "Eventing Jurypräsident", fr: "Eventing Président de jury", it: "Eventing Presidente di giuria", order: Some(7040), fei: false },
    Title { id: 103, code: "MPBCI", de: "Eventing Parcoursbauer Cross I", fr: "Eventing Constructeur de parcours de cross I", it: "Eventing Costrutt. di percorsi cross I", order: Some(12130), fei: false },
    Title { id: 106, code: "MPBFEII", de: "Eventing Parcoursbauer International FEI", fr: "Eventing Constructeur de parcours international FEI", it: "Eventing Costruttore di percorsi internazionale FEI", order: Some(12110), fei: true },
    Title { id: 111, code: "MRIFEII", de: "Eventing Richter International FEI", fr: "Eventing Juge international FEI", it: "Eventing Giudice internazionale FEI", order: Some(7020), fei: true },
    Title { id: 113, code: "MTDI", de: "Eventing Technischer Delegierter I", fr: "Eventing Délégué technique I", it: "Eventing Delegato tecnico I", order: Some(7100), fei: false },
    Title { id: 116, code: "MTDFEII", de: "Eventing Tech. Delegierter Intern. FEI", fr: "Eventing Délégué technique international FEI", it: "Eventing Delegato tecnico internazionale FEI", order: Some(7080), fei: true },
    Title { id: 127, code: "PBZ", de: "Passbeauftragter Zucht/Zoll", fr: "Agré établ. passeport élevage/zoll", it: "Incaricato ai passaporti allevamento/dogana", order: Some(0), fei: false },
    Title { id: 128, code: "RECSTFEI", de: "Steward FEI Reining", fr: "Steward FEI Reining", it: "Steward FEI Reining", order: Some(13180), fei: true },
    Title { id: 130, code: "REIFEII", de: "Reiningrichter International FEI", fr: "Juge international de reining", it: "Giudice internazionale reining", order: Some(10045), fei: true },
    Title { id: 145, code: "SCSTFEI", de: "Steward FEI Jumping", fr: "Steward FEI Jumping", it: "Steward FEI Jumping", order: Some(13080), fei: true },
    Title { id: 147, code: "SGSTFEI", de: "General Steward FEI Jumping", fr: "General Steward FEI Jumping", it: "General Steward FEI Jumping", order: Some(13070), fei: true },
    Title { id: 149, code: "SJP", de: "Springen Jurypräsident", fr: "Président de jury saut", it: "Presidente di giuria di salto", order: Some(6040), fei: false },
    Title { id: 150, code: "SPBASS", de: "Springen Parcoursbauer Assistent", fr: "Assistant constructeur de parcours saut", it: "Assistente costruttore di percorsi salto", order: Some(12090), fei: false },
    Title { id: 152, code: "SPBB I", de: "Springen Parcoursbauer Brevet I", fr: "Constructeur de parcours Brevet I", it: "Costruttore di pecorsi Brevetto I salto", order: Some(12042), fei: false },
    Title { id: 153, code: "SPBB II", de: "Springen Parcoursbauer Brevet II", fr: "Constructeur de parcours Brevet II", it: "Costruttore di pecorsi Brevetto II salto", order: Some(12041), fei: false },
    Title { id: 154, code: "SPBB III", de: "Springen Parcoursbauer Brevet III", fr: "Constructeur de parcours Brevet III saut", it: "Costruttore di pecorsi Brevetto III salto", order: Some(12040), fei: false },
    Title { id: 161, code: "SPBFEII", de: "Springen Parcoursbauer International FEI", fr: "Constructeur de parcours international FEI saut", it: "Costruttore di percorsi internazionale FEI salto", order: Some(12020), fei: true },
    Title { id: 166, code: "SRI", de: "Springrichter National", fr: "Juge national de saut", it: "Giudice nazionale di salto", order: Some(6050), fei: false },
    Title { id: 167, code: "SRIA", de: "Springrichter Anwärter", fr: "Aspirant-juge national de saut", it: "Aspirante giudice di salto", order: Some(6060), fei: false },
    Title { id: 169, code: "SRIFEII", de: "Springrichter International FEI", fr: "Juge international FEI saut", it: "Giudice internazionale FEI salto", order: Some(6020), fei: true },
    Title { id: 173, code: "SO TR LS / BT", de: "SO Trainer Leistungssport / Berufstrainer", fr: "SO Entraîneur sport de performance / Entraîneur professionnel", it: "SO Allenatore sport di prestazione / Allenatore professionale", order: Some(2060), fei: false },
    Title { id: 175, code: "TRSVP", de: "SVPS Trainer", fr: "Entraîneur FSSE", it: "Allenatore FSSE", order: Some(15010), fei: false },
    Title { id: 176, code: "TRVER", de: "Vereinstrainer", fr: "Entraîneur de société", it: "Allenatore di società", order: Some(15020), fei: false },
    Title { id: 177, code: "TRVERFA", de: "Vereinstrainer Fahren", fr: "Entraîneur de société attelage", it: "Allenatore di società attacchi", order: Some(15021), fei: false },
    Title { id: 178, code: "TRVERG", de: "Vereinstrainer IPV", fr: "Entraîneur de société IPV", it: "Allenatore di società IPV", order: Some(15022), fei: false },
    Title { id: 179, code: "TRVERW", de: "Vereinstrainer SWRA", fr: "Entraîneur de société SWRA", it: "Allenatore di società SWRA", order: Some(15023), fei: false },
    Title { id: 180, code: "VCSTFEI", de: "Steward FEI Vaulting", fr: "Steward FEI Vaulting", it: "Steward FEI Vaulting", order: Some(13210), fei: true },
    Title { id: 186, code: "VETDFEIE", de: "Veterinär Concours EV Dressur FEI", fr: "Vétérinaire de concours EV FEI dressage", it: "Veterinario del concorso EV FEI dressage", order: Some(14020), fei: true },
    Title { id: 187, code: "VETDIFEIE", de: "Veterinär Concours EV Endurance FEI", fr: "Vétérinaire de concours EV FEI raids d'endurance", it: "Veterinario del concorso EV FEI endurance", order: Some(14060), fei: true },
    Title { id: 188, code: "VETFEIC", de: "Veterinär Kontakt CV FEI", fr: "Vétérinaire de contact CV FEI", it: "Veterinario di contatto CV FEI", order: Some(14010), fei: true },
    Title { id: 189, code: "VETFFEIE", de: "Veterinär Concours EV Fahren FEI", fr: "Vétérinaire de concours EV FEI attelage", it: "Veterinario del concorso EV FEI attacchi", order: Some(14050), fei: true },
    Title { id: 190, code: "VETMFEIE", de: "Veterinär Concours EV FEI Eventing", fr: "Vétérinaire de concours EV FEI Eventing", it: "Veterinario del concorso EV FEI Eventing", order: Some(14040), fei: true },
    Title { id: 192, code: "VETSFEIE", de: "Veterinär Concours EV Springen FEI", fr: "Vétérinaire de concours EV FEI saut", it: "Veterinario del concorso EV FEI salto", order: Some(14030), fei: true },
    Title { id: 193, code: "VETSVP", de: "Pass Veterinär", fr: "Vétérinaire pour les passeports", it: "Veterinario per i passaporti", order: Some(14080), fei: false },
    Title { id: 194, code: "VETVFEIE", de: "Veterinär Concours EV Voltige FEI", fr: "Vétérinaire de concours EV FEI voltige", it: "Veterinario del concorso EV FEI volteggio", order: Some(14080), fei: true },
    Title { id: 198, code: "VKRI", de: "Vierkampfrichter National", fr: "Juge tétrathlon national", it: "Giudice nazionale di tetrathlon", order: Some(9050), fei: false },
    Title { id: 204, code: "VRI", de: "Voltigerichter National", fr: "Juge de voltige national", it: "Giudice nazionale di volteggio", order: Some(9040), fei: false },
    Title { id: 206, code: "VRIFEII", de: "Voltigerichter International FEI", fr: "Juge international FEI voltige", it: "Giudice internazionale FEI volteggio", order: Some(9020), fei: true },
    Title { id: 208, code: "DRIFEIPED2*", de: "Dressurrichter Para Equestre FEI", fr: "Dressage Judge Para Equestre FEI", it: "Giudice FEI Para-Equestrian dressage", order: Some(5045), fei: true },
    Title { id: 209, code: "VETTT", de: "Turniertierarzt", fr: "Vétérinaire de concours", it: "Veterinario del concorso", order: Some(14081), fei: false },
    Title { id: 211, code: "J+S LEIT A R", de: "J+S Leiter:in A Reiten", fr: "J+S Moniteur·trice A Équitation", it: "G+S Monitore/trice A Equitazione", order: Some(1074), fei: false },
    Title { id: 212, code: "J+S LEIT B VTR", de: "J+S Leiter/in B / Vereinstrainer :in Reiten", fr: "J+S Moniteur/trice B / Entraîneur·e de société Équitation", it: "G+S Monitore/trice B / Allenatore/trice di club Equitazione", order: Some(1075), fei: false },
    Title { id: 213, code: "TRNW", de: "Nachwuchstrainer", fr: "Entraîneur reléve", it: "Allenatore promozione giovanile", order: Some(15020), fei: false },
    Title { id: 217, code: "MPBCII", de: "Eventing Parcoursbauer Cross II", fr: "Eventing Constructeur de parcours de cross II", it: "Eventing Costruttore di percorsi cross II", order: Some(12132), fei: false },
    Title { id: 218, code: "MTDII", de: "Eventing Technischer Delegierter II", fr: "Eventing Délégué technique II", it: "Eventing Delegato tecnico II", order: Some(7105), fei: false },
    Title { id: 220, code: "DTTD", de: "Dressur Tages-TD", fr: "DT du jour Dressage", it: "Delegato Tecnico del giorno Dressage", order: None, fei: false },
    Title { id: 221, code: "DRA", de: "Dressurrichter Ausland", fr: "Juge de dressage étranger", it: "Giudice di dressage estero", order: Some(5015), fei: false },
    Title { id: 222, code: "J+S LEIT KIND", de: "J+S Leiter/in Kindersport Pferdesport", fr: "J+S Moniteur/trice Sport des Enfants sports équestres", it: "G+S Monitore/trice Sport per Bambini degli sport equestri", order: Some(1076), fei: false },
    Title { id: 223, code: "J+S EXP KIND", de: "J+S Experte/in Kindersport-Pferdesport", fr: "J+S Expert Sport des Enfants Sport èquestre", it: "G+S Esperto/a per Bambini degli sport equestri", order: Some(2054), fei: false },
    Title { id: 224, code: "J+S AV P/R", de: "J+S Ausbildungsverantwortliche/r Pferdesport/Reiten", fr: "J+S Responsable de formation Sports équestres/Équitation", it: "G+S Responsabile della formazione Sport equestri / Equitazione", order: None, fei: false },
    Title { id: 225, code: "DRITdJ", de: "Dressurrichter Tag der Jugend", fr: "Juge de dressage journée de la jeunesse", it: "Giudice di dressage Giornata della Gioventù", order: Some(5120), fei: false },
    Title { id: 226, code: "EXR", de: "Experte Reiten", fr: "Expert équitation", it: "Esperto equitazione", order: Some(11045), fei: false },
    Title { id: 227, code: "EXF", de: "Experte Fahren", fr: "Expert attelage", it: "Esperto attacchi", order: Some(11046), fei: false },
    Title { id: 228, code: "ABGAR", de: "Ausbilder Grundausbildung Reiten", fr: "Instructeur formation de base équitation", it: "Istruttore Formazione di base equitazione", order: Some(11058), fei: false },
    Title { id: 229, code: "ABGAF", de: "Ausbilder Grundausbildung Fahren", fr: "Instructeur formation de base attelage", it: "Istruttore Formazione di base attacchi", order: Some(11059), fei: false },
    Title { id: 231, code: "DRAA", de: "Dressur Aufsichtsperson Abreitplatz", fr: "Dressage Responsable place d’échauffement", it: "Supervisore in campo prova dressage", order: Some(5012), fei: false },
    Title { id: 232, code: "ABGAV", de: "Ausbilder Grundausbildung Voltige", fr: "Instructeur formation de base voltige", it: "Istruttore Formazione di base volteggio", order: Some(11059), fei: false },
    Title { id: 233, code: "EXV", de: "Experte Voltige", fr: "Expert voltige", it: "Esperto volteggio", order: Some(11059), fei: false },
    Title { id: 235, code: "FATD", de: "Fahren Technischer Delegierter", fr: "Attelage Délégué technique", it: "Delegato tecnico Attacchi", order: Some(8036), fei: false },
    Title { id: 236, code: "FAJP", de: "Fahren Jurypräsident", fr: "Attelage Président de jury", it: "Presidente di giuria Attacchi", order: Some(8035), fei: false },
    Title { id: 237, code: "DRIL+", de: "Dressurrichter L+", fr: "Juge de dressage L+", it: "Giudice di dressage L+", order: Some(5091), fei: false },
    Title { id: 238, code: "ENJPD", de: "Endurance Jurypräsident und Tech. Delegierter", fr: "Endurance Président de jury et Délégué technique", it: "Presidente della giuria di endurance e Delegato tecnico", order: Some(8037), fei: false },
    Title { id: 239, code: "RAA", de: "Aufsichtsperson Abreitplatz", fr: "Responsable place d’échauffement", it: "Supervisore in campo prova", order: Some(5012), fei: false },
    Title { id: 240, code: "ST", de: "Steward", fr: "Steward", it: "Steward", order: None, fei: false },
    Title { id: 256, code: "DRIPD", de: "Dressurrichter Para", fr: "Juge de dressage Para", it: "Giudice die dressage Para", order: None, fei: false },
    Title { id: 257, code: "STRIE", de: "Stilrichter Einsteigerprüfungen", fr: "Juge de style épreuves d'initiation", it: "Giudice di stile gare introduttivi", order: Some(6071), fei: false },
    Title { id: 265, code: "TLD", de: "Dressur Turnierleiter", fr: "Responsable de concours dressage", it: "Responsabile del concorso dressage", order: None, fei: false },
    Title { id: 266, code: "EXRV", de: "Experte Reiten verantwortlich", fr: "Expert équitation responsable", it: "Esperto equitazione responsabile", order: Some(11047), fei: false },
    Title { id: 267, code: "EXFV", de: "Experte Fahren verantwortlich", fr: "Expert attelage responsable", it: "Esperto attacchi responsabile", order: Some(11048), fei: false },
    Title { id: 269, code: "STIRILI", de: "Stilrichter Stilprüfungen Lizenz", fr: "Juge de style épreuves de style licence", it: "Giudice di stile gare stile licenza", order: Some(6072), fei: false },
    Title { id: 271, code: "J+S LEIT B VTV", de: "J+S Leiter:in B / Vereinstrainer:in Voltige", fr: "J+S Moniteur·trice B / Entraîneur·e de société Voltige", it: "G+S Monitore/trice B / Allenatore/trice di club Volteggio", order: None, fei: false },
    Title { id: 272, code: "J+S LEIT A V", de: "J+S Leiter:in A Voltige", fr: "J+S Moniteur·trice A Voltige", it: "G+S Monitore/trice A Volteggio", order: None, fei: false },
    Title { id: 273, code: "J+S EXP R", de: "J+S Expert:in Reiten", fr: "J+S Expert·e Équitation", it: "G+S Esperto/a Equitazione", order: None, fei: false },
    Title { id: 274, code: "J+S EXP V", de: "J+S Expert:in Voltige", fr: "J+S Expert·e Voltige", it: "G+S Esperto/a Volteggio", order: None, fei: false },
    Title { id: 275, code: "J+S AV V", de: "J+S Ausbildungsverantwortliche Voltige", fr: "J+S Responsable de formation Voltige", it: "G+S Responsabile della formazione Volteggio", order: None, fei: false },
    Title { id: 276, code: "J+S LEIT R", de: "J+S Leiter:in Reiten", fr: "J+S Moniteur·trice Équitation", it: "G+S Monitore/trice Equitazione", order: None, fei: false },
    Title { id: 277, code: "J+S LEIT V", de: "J+S Leiter:in Voltige", fr: "J+S Moniteur·trice Voltige", it: "G+S Monitore/trice Volteggio", order: None, fei: false },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn found_by_german() {
        let judge = title("Springrichter National").unwrap();
        assert_eq!((judge.code, judge.fr, judge.it), ("SRI", "Juge national de saut", "Giudice nazionale di salto"));
        assert_eq!(title(" J+S COACH ").unwrap().it, "G+S COACH");
        assert!(title("Coach Jugend & Sport").is_none());
        // Every title once.
        assert!(TITLES.iter().all(|t| TITLES.iter().filter(|u| u.de == t.de).count() == 1));
    }
}
