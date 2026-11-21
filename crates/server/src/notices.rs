//! The e-mails about shows (in the recipient's language: French, German or Italian): a request for the owner, the answer
//! for who asked, access given without a request. Plain text; the apps do the rest.

use crate::lang::Lang;

/// (subject, body)
pub type Mail = (String, String);

fn signed(lang: Lang, greeting_name: &str, text: &str) -> String {
    match lang {
        Lang::Fr => format!("Bonjour {greeting_name},\n\n{text}\n\nEquiEvents\n"),
        Lang::De => format!("Guten Tag {greeting_name}\n\n{text}\n\nEquiEvents\n"),
        Lang::It => format!("Buongiorno {greeting_name},\n\n{text}\n\nEquiEvents\n"),
    }
}

/// For the owner: someone asks for access to their show, or for the show itself.
pub fn request_made(lang: Lang, owner: &str, asker: &str, asker_email: &str, show: &str, ownership: bool) -> Mail {
    let (subject, text) = match (lang, ownership) {
        (Lang::Fr, false) => (
            format!("EquiEvents : demande d’accès à {show}"),
            format!("{asker} ({asker_email}) demande à travailler sur le concours {show}.\n\nOuvrez EquiEvents pour accepter ou refuser."),
        ),
        (Lang::Fr, true) => (
            format!("EquiEvents : demande de reprise de {show}"),
            format!("{asker} ({asker_email}) demande à devenir responsable du concours {show}.\n\nOuvrez EquiEvents pour accepter ou refuser. Si vous acceptez, vous choisissez de rester sur le concours comme collaborateur·trice ou de le quitter."),
        ),
        (Lang::De, false) => (
            format!("EquiEvents: Zugriffsanfrage für {show}"),
            format!("{asker} ({asker_email}) möchte am Turnier {show} mitarbeiten.\n\nÖffnen Sie EquiEvents, um anzunehmen oder abzulehnen."),
        ),
        (Lang::De, true) => (
            format!("EquiEvents: Übernahmeanfrage für {show}"),
            format!("{asker} ({asker_email}) möchte Verantwortliche:r des Turniers {show} werden.\n\nÖffnen Sie EquiEvents, um anzunehmen oder abzulehnen. Wenn Sie annehmen, wählen Sie, ob Sie als Mitarbeiter:in beim Turnier bleiben oder es verlassen."),
        ),
        (Lang::It, false) => (
            format!("EquiEvents: richiesta di accesso a {show}"),
            format!("{asker} ({asker_email}) chiede di lavorare sul concorso {show}.\n\nAprite EquiEvents per accettare o rifiutare."),
        ),
        (Lang::It, true) => (
            format!("EquiEvents: richiesta di subentro per {show}"),
            format!("{asker} ({asker_email}) chiede di diventare responsabile del concorso {show}.\n\nAprite EquiEvents per accettare o rifiutare. Se accettate, scegliete se restare sul concorso come collaboratore/trice o lasciarlo."),
        ),
    };
    (subject, signed(lang, owner, &text))
}

/// For who asked: the owner's answer.
pub fn request_answered(lang: Lang, asker: &str, show: &str, ownership: bool, accepted: bool) -> Mail {
    let (subject, text) = match (lang, ownership, accepted) {
        (Lang::Fr, false, true) => (format!("EquiEvents : accès à {show} accordé"), format!("Vous pouvez maintenant travailler sur le concours {show} dans EquiEvents.")),
        (Lang::Fr, true, true) => (format!("EquiEvents : vous êtes responsable de {show}"), format!("Vous êtes maintenant responsable du concours {show} : vous décidez qui y travaille.")),
        (Lang::Fr, _, false) => (format!("EquiEvents : demande pour {show} refusée"), format!("Votre demande pour le concours {show} a été refusée.")),
        (Lang::De, false, true) => (format!("EquiEvents: Zugriff auf {show} erteilt"), format!("Sie können jetzt in EquiEvents am Turnier {show} mitarbeiten.")),
        (Lang::De, true, true) => (format!("EquiEvents: Sie sind Verantwortliche:r von {show}"), format!("Sie sind jetzt Verantwortliche:r des Turniers {show}: Sie entscheiden, wer daran mitarbeitet.")),
        (Lang::De, _, false) => (format!("EquiEvents: Anfrage für {show} abgelehnt"), format!("Ihre Anfrage für das Turnier {show} wurde abgelehnt.")),
        (Lang::It, false, true) => (format!("EquiEvents: accesso a {show} concesso"), format!("Ora potete lavorare sul concorso {show} in EquiEvents.")),
        (Lang::It, true, true) => (format!("EquiEvents: siete responsabili di {show}"), format!("Ora siete responsabili del concorso {show}: decidete voi chi ci lavora.")),
        (Lang::It, _, false) => (format!("EquiEvents: richiesta per {show} rifiutata"), format!("La vostra richiesta per il concorso {show} è stata rifiutata.")),
    };
    (subject, signed(lang, asker, &text))
}

/// For someone the owner let work on their show without a request.
pub fn access_given(lang: Lang, name: &str, owner: &str, show: &str) -> Mail {
    let (subject, text) = match lang {
        Lang::Fr => (format!("EquiEvents : accès à {show}"), format!("{owner} vous a donné accès au concours {show} : il apparaît dans EquiEvents.")),
        Lang::De => (format!("EquiEvents: Zugriff auf {show}"), format!("{owner} hat Ihnen Zugriff auf das Turnier {show} gegeben: Es erscheint in EquiEvents.")),
        Lang::It => (format!("EquiEvents: accesso a {show}"), format!("{owner} vi ha dato accesso al concorso {show}: appare in EquiEvents.")),
    };
    (subject, signed(lang, name, &text))
}

/// For whom the owner gave the show while leaving it.
pub fn handed_over(lang: Lang, name: &str, owner: &str, show: &str) -> Mail {
    let (subject, text) = match lang {
        Lang::Fr => (format!("EquiEvents : vous êtes responsable de {show}"), format!("{owner} vous a remis le concours {show} et l’a quitté : vous en êtes maintenant responsable.")),
        Lang::De => (format!("EquiEvents: Sie sind Verantwortliche:r von {show}"), format!("{owner} hat Ihnen das Turnier {show} übergeben und es verlassen: Sie sind jetzt Verantwortliche:r.")),
        Lang::It => (format!("EquiEvents: siete responsabili di {show}"), format!("{owner} vi ha affidato il concorso {show} e l’ha lasciato: ora ne siete responsabili.")),
    };
    (subject, signed(lang, name, &text))
}

/// For who worked on a show its owner released: deleted from equievents.ch.
pub fn released(lang: Lang, name: &str, owner: &str, show: &str) -> Mail {
    let (subject, text) = match lang {
        Lang::Fr => (
            format!("EquiEvents : {show} supprimé"),
            format!("{owner} a supprimé le concours {show} d’equievents.ch : vous n’y avez plus accès. Une copie sur vos ordinateurs y reste, sans être partagée."),
        ),
        Lang::De => (
            format!("EquiEvents: {show} gelöscht"),
            format!("{owner} hat das Turnier {show} auf equievents.ch gelöscht: Sie haben keinen Zugriff mehr. Eine Kopie auf Ihren Computern bleibt dort, ohne geteilt zu werden."),
        ),
        Lang::It => (
            format!("EquiEvents: {show} eliminato"),
            format!("{owner} ha eliminato il concorso {show} da equievents.ch: non vi avete più accesso. Una copia sui vostri computer vi resta, senza essere condivisa."),
        ),
    };
    (subject, signed(lang, name, &text))
}

/// For who worked on a show whose owner's account an administrator deleted: deleted too.
pub fn account_deleted(lang: Lang, name: &str, owner: &str, show: &str) -> Mail {
    let (subject, text) = match lang {
        Lang::Fr => (
            format!("EquiEvents : {show} supprimé"),
            format!("Le compte de {owner} a été supprimé d’equievents.ch, et avec lui le concours {show} : vous n’y avez plus accès. Une copie sur vos ordinateurs y reste, sans être partagée."),
        ),
        Lang::De => (
            format!("EquiEvents: {show} gelöscht"),
            format!("Das Konto von {owner} wurde auf equievents.ch gelöscht, und mit ihm das Turnier {show}: Sie haben keinen Zugriff mehr. Eine Kopie auf Ihren Computern bleibt dort, ohne geteilt zu werden."),
        ),
        Lang::It => (
            format!("EquiEvents: {show} eliminato"),
            format!("L’account di {owner} è stato eliminato da equievents.ch, e con esso il concorso {show}: non vi avete più accesso. Una copia sui vostri computer vi resta, senza essere condivisa."),
        ),
    };
    (subject, signed(lang, name, &text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worded() {
        let (subject, body) = request_made(Lang::De, "Anne", "Bruno", "bruno@example.ch", "CSN Chalet-à-Gobet", true);
        assert_eq!(subject, "EquiEvents: Übernahmeanfrage für CSN Chalet-à-Gobet");
        assert!(body.starts_with("Guten Tag Anne\n") && body.contains("Bruno (bruno@example.ch)"));
        assert_eq!(request_answered(Lang::Fr, "Bruno", "CSN", false, false).0, "EquiEvents : demande pour CSN refusée");
        assert!(access_given(Lang::Fr, "Dora", "Anne", "CSN").1.contains("Anne vous a donné accès au concours CSN"));
        let (subject, body) = request_made(Lang::It, "Anna", "Bruno", "bruno@example.ch", "CSN Lugano", false);
        assert!(subject == "EquiEvents: richiesta di accesso a CSN Lugano" && body.starts_with("Buongiorno Anna,\n"));
    }
}
