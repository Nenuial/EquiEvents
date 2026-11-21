//! The e-mails about shows, in the recipient's language (French, German or Italian): a request
//! for the owner, the answer for who asked, access given without a request, a show handed over,
//! released or deleted with its owner's account, another owner set by an administrator. Each a
//! [`Letter`]: plain text and HTML together (`letter`); a button to EquiEvents where there is
//! something to do there.

use crate::lang::Lang;
pub use crate::letter::Letter;

/// The words of each language (so every e-mail is written in the three side by side).
fn say(lang: Lang, fr: String, de: String, it: String) -> String {
    match lang {
        Lang::Fr => fr,
        Lang::De => de,
        Lang::It => it,
    }
}

/// For the owner: someone asks for access to their show, or for the show itself.
pub fn request_made(lang: Lang, owner: &str, asker: &str, asker_email: &str, show: &str, ownership: bool) -> Letter {
    let (topic, asks, then) = if ownership {
        (
            say(lang, format!("demande de reprise de {show}"), format!("Übernahmeanfrage für {show}"), format!("richiesta di subentro per {show}")),
            say(
                lang,
                format!("{asker} ({asker_email}) demande à devenir responsable du concours {show}."),
                format!("{asker} ({asker_email}) möchte Verantwortliche:r des Turniers {show} werden."),
                format!("{asker} ({asker_email}) chiede di diventare responsabile del concorso {show}."),
            ),
            lang.pick(
                "Ouvrez EquiEvents pour accepter ou refuser. Si vous acceptez, vous choisissez de rester sur le concours comme collaborateur·trice ou de le quitter.",
                "Öffnen Sie EquiEvents, um anzunehmen oder abzulehnen. Wenn Sie annehmen, wählen Sie, ob Sie als Mitarbeiter:in beim Turnier bleiben oder es verlassen.",
                "Aprite EquiEvents per accettare o rifiutare. Se accettate, scegliete se restare sul concorso come collaboratore/trice o lasciarlo.",
            ),
        )
    } else {
        (
            say(lang, format!("demande d’accès à {show}"), format!("Zugriffsanfrage für {show}"), format!("richiesta di accesso a {show}")),
            say(
                lang,
                format!("{asker} ({asker_email}) demande à travailler sur le concours {show}."),
                format!("{asker} ({asker_email}) möchte am Turnier {show} mitarbeiten."),
                format!("{asker} ({asker_email}) chiede di lavorare sul concorso {show}."),
            ),
            lang.pick("Ouvrez EquiEvents pour accepter ou refuser.", "Öffnen Sie EquiEvents, um anzunehmen oder abzulehnen.", "Aprite EquiEvents per accettare o rifiutare."),
        )
    };
    Letter::new(lang, topic, owner, vec![asks, then.into()]).open_app()
}

/// For who asked: the owner's answer.
pub fn request_answered(lang: Lang, asker: &str, show: &str, ownership: bool, accepted: bool) -> Letter {
    let (topic, text) = match (ownership, accepted) {
        (false, true) => (
            say(lang, format!("accès à {show} accordé"), format!("Zugriff auf {show} erteilt"), format!("accesso a {show} concesso")),
            say(
                lang,
                format!("Vous pouvez maintenant travailler sur le concours {show} dans EquiEvents."),
                format!("Sie können jetzt in EquiEvents am Turnier {show} mitarbeiten."),
                format!("Ora potete lavorare sul concorso {show} in EquiEvents."),
            ),
        ),
        (true, true) => (
            say(lang, format!("vous êtes responsable de {show}"), format!("Sie sind Verantwortliche:r von {show}"), format!("siete responsabili di {show}")),
            say(
                lang,
                format!("Vous êtes maintenant responsable du concours {show} : vous décidez qui y travaille."),
                format!("Sie sind jetzt Verantwortliche:r des Turniers {show}: Sie entscheiden, wer daran mitarbeitet."),
                format!("Ora siete responsabili del concorso {show}: decidete voi chi ci lavora."),
            ),
        ),
        (_, false) => (
            say(lang, format!("demande pour {show} refusée"), format!("Anfrage für {show} abgelehnt"), format!("richiesta per {show} rifiutata")),
            say(
                lang,
                format!("Votre demande pour le concours {show} a été refusée."),
                format!("Ihre Anfrage für das Turnier {show} wurde abgelehnt."),
                format!("La vostra richiesta per il concorso {show} è stata rifiutata."),
            ),
        ),
    };
    let letter = Letter::new(lang, topic, asker, vec![text]);
    if accepted {
        letter.open_app()
    } else {
        letter
    }
}

/// For someone the owner let work on their show without a request.
pub fn access_given(lang: Lang, name: &str, owner: &str, show: &str) -> Letter {
    let topic = say(lang, format!("accès à {show}"), format!("Zugriff auf {show}"), format!("accesso a {show}"));
    let text = say(
        lang,
        format!("{owner} vous a donné accès au concours {show} : il apparaît dans EquiEvents."),
        format!("{owner} hat Ihnen Zugriff auf das Turnier {show} gegeben: Es erscheint in EquiEvents."),
        format!("{owner} vi ha dato accesso al concorso {show}: appare in EquiEvents."),
    );
    Letter::new(lang, topic, name, vec![text]).open_app()
}

/// For whom the owner gave the show while leaving it.
pub fn handed_over(lang: Lang, name: &str, owner: &str, show: &str) -> Letter {
    let topic = say(lang, format!("vous êtes responsable de {show}"), format!("Sie sind Verantwortliche:r von {show}"), format!("siete responsabili di {show}"));
    let text = say(
        lang,
        format!("{owner} vous a remis le concours {show} et l’a quitté : vous en êtes maintenant responsable."),
        format!("{owner} hat Ihnen das Turnier {show} übergeben und es verlassen: Sie sind jetzt Verantwortliche:r."),
        format!("{owner} vi ha affidato il concorso {show} e l’ha lasciato: ora ne siete responsabili."),
    );
    Letter::new(lang, topic, name, vec![text]).open_app()
}

/// What stays on the reader's computers when a show is gone from equievents.ch for them.
fn copy_stays(lang: Lang) -> &'static str {
    lang.pick(
        "Une copie sur vos ordinateurs y reste, sans être partagée.",
        "Eine Kopie auf Ihren Computern bleibt dort, ohne geteilt zu werden.",
        "Una copia sui vostri computer vi resta, senza essere condivisa.",
    )
}

/// For who worked on a show its owner released: deleted from equievents.ch.
pub fn released(lang: Lang, name: &str, owner: &str, show: &str) -> Letter {
    let topic = say(lang, format!("{show} supprimé"), format!("{show} gelöscht"), format!("{show} eliminato"));
    let text = say(
        lang,
        format!("{owner} a supprimé le concours {show} d’equievents.ch : vous n’y avez plus accès."),
        format!("{owner} hat das Turnier {show} auf equievents.ch gelöscht: Sie haben keinen Zugriff mehr."),
        format!("{owner} ha eliminato il concorso {show} da equievents.ch: non vi avete più accesso."),
    );
    Letter::new(lang, topic, name, vec![text]).after(copy_stays(lang))
}

/// For the owner of a show put on equievents.ch with a key Swiss Equestrian could not check (a
/// past event's, from its Rosson export), when someone came with a key it accepts for the event:
/// the show stays, no longer the event's.
pub fn unlinked(lang: Lang, name: &str, show: &str) -> Letter {
    let topic = say(lang, format!("{show} n’est plus lié à son événement"), format!("{show} ist nicht mehr mit seiner Veranstaltung verbunden"), format!("{show} non è più legato al suo evento"));
    let text = say(
        lang,
        format!("Swiss Equestrian a accepté une autre clé pour l’événement de votre concours {show}. Votre concours a été mis sur equievents.ch avec une clé qu’il ne pouvait pas vérifier : il n’est donc plus celui de l’événement."),
        format!("Swiss Equestrian hat für die Veranstaltung Ihres Turniers {show} einen anderen Schlüssel akzeptiert. Ihr Turnier wurde mit einem Schlüssel auf equievents.ch gestellt, den es nicht prüfen konnte: Es ist darum nicht mehr das Turnier der Veranstaltung."),
        format!("Swiss Equestrian ha accettato un’altra chiave per l’evento del vostro concorso {show}. Il vostro concorso era stato messo su equievents.ch con una chiave che non poteva verificare: non è quindi più quello dell’evento."),
    );
    let after = lang.pick(
        "Il reste sur equievents.ch et sur vos ordinateurs, tel quel, sans lien avec Swiss Equestrian. Si c’est une erreur, écrivez au support d’equievents.ch.",
        "Es bleibt unverändert auf equievents.ch und auf Ihren Computern, ohne Verbindung zu Swiss Equestrian. Falls das ein Irrtum ist, schreiben Sie dem Support von equievents.ch.",
        "Resta così com’è su equievents.ch e sui vostri computer, senza legame con Swiss Equestrian. Se si tratta di un errore, scrivete al supporto di equievents.ch.",
    );
    Letter::new(lang, topic, name, vec![text]).after(after)
}

/// For who worked on a show whose owner's account an administrator deleted: deleted too.
pub fn account_deleted(lang: Lang, name: &str, owner: &str, show: &str) -> Letter {
    let topic = say(lang, format!("{show} supprimé"), format!("{show} gelöscht"), format!("{show} eliminato"));
    let text = say(
        lang,
        format!("Le compte de {owner} a été supprimé d’equievents.ch, et avec lui le concours {show} : vous n’y avez plus accès."),
        format!("Das Konto von {owner} wurde auf equievents.ch gelöscht, und mit ihm das Turnier {show}: Sie haben keinen Zugriff mehr."),
        format!("L’account di {owner} è stato eliminato da equievents.ch, e con esso il concorso {show}: non vi avete più accesso."),
    );
    Letter::new(lang, topic, name, vec![text]).after(copy_stays(lang))
}

/// For whom an administrator of equievents.ch made a show's owner.
pub fn owner_set(lang: Lang, name: &str, show: &str) -> Letter {
    let topic = say(lang, format!("vous êtes responsable de {show}"), format!("Sie sind Verantwortliche:r von {show}"), format!("siete responsabili di {show}"));
    let text = say(
        lang,
        format!("L’équipe d’equievents.ch vous a nommé·e responsable du concours {show} : vous décidez qui y travaille."),
        format!("Das Team von equievents.ch hat Sie als Verantwortliche:n des Turniers {show} eingesetzt: Sie entscheiden, wer daran mitarbeitet."),
        format!("Il team di equievents.ch vi ha nominati responsabili del concorso {show}: decidete voi chi ci lavora."),
    );
    Letter::new(lang, topic, name, vec![text]).open_app()
}

/// For a show's previous owner, when an administrator of equievents.ch gave it to someone else
/// (`stays`: they stay as a collaborator).
pub fn owner_replaced(lang: Lang, name: &str, owner: &str, show: &str, stays: bool) -> Letter {
    let topic = say(lang, format!("nouveau responsable pour {show}"), format!("neue Verantwortliche oder neuer Verantwortlicher für {show}"), format!("nuovo responsabile per {show}"));
    let what = say(
        lang,
        format!("L’équipe d’equievents.ch a nommé {owner} responsable du concours {show}."),
        format!("Das Team von equievents.ch hat {owner} als Verantwortliche:n des Turniers {show} eingesetzt."),
        format!("Il team di equievents.ch ha nominato {owner} responsabile del concorso {show}."),
    );
    let after = if stays {
        lang.pick("Vous y restez comme collaborateur·trice.", "Sie arbeiten weiterhin als Mitarbeiter:in daran.", "Restate sul concorso come collaboratore/trice.")
    } else {
        lang.pick(
            "Vous n’y avez plus accès ; une copie sur vos ordinateurs y reste, sans être partagée.",
            "Sie haben keinen Zugriff mehr; eine Kopie auf Ihren Computern bleibt dort, ohne geteilt zu werden.",
            "Non vi avete più accesso; una copia sui vostri computer vi resta, senza essere condivisa.",
        )
    };
    let letter = Letter::new(lang, topic, name, vec![format!("{what} {after}")]);
    if stays {
        letter.open_app()
    } else {
        letter
    }
}

/// Every e-mail of equievents.ch, as an example, in `lang` (the preview).
pub fn examples(lang: Lang, site: &str) -> Vec<(&'static str, Letter)> {
    let show = "CSI Chalet-à-Gobet";
    vec![
        ("reset", crate::site::reset_mail(lang, site, "Anne Rochat", "anne@example.ch", "0123456789abcdef")),
        ("request-access", request_made(lang, "Anne Rochat", "Bruno Keller", "bruno@example.ch", show, false)),
        ("request-ownership", request_made(lang, "Anne Rochat", "Bruno Keller", "bruno@example.ch", show, true)),
        ("access-accepted", request_answered(lang, "Bruno Keller", show, false, true)),
        ("ownership-accepted", request_answered(lang, "Bruno Keller", show, true, true)),
        ("request-refused", request_answered(lang, "Bruno Keller", show, false, false)),
        ("access-given", access_given(lang, "Carla Bianchi", "Anne Rochat", show)),
        ("handed-over", handed_over(lang, "Bruno Keller", "Anne Rochat", show)),
        ("released", released(lang, "Bruno Keller", "Anne Rochat", show)),
        ("account-deleted", account_deleted(lang, "Bruno Keller", "Anne Rochat", show)),
        ("unlinked", unlinked(lang, "Bruno Keller", show)),
        ("owner-set", owner_set(lang, "Carla Bianchi", show)),
        ("owner-replaced-stays", owner_replaced(lang, "Anne Rochat", "Carla Bianchi", show, true)),
        ("owner-replaced-leaves", owner_replaced(lang, "Anne Rochat", "Carla Bianchi", show, false)),
    ]
}

/// A page showing every e-mail in the three languages, light and dark side by side
/// (`equievents-server mail-preview`).
pub fn preview(site: &str) -> String {
    let mut sections = String::new();
    for lang in Lang::ALL {
        for (name, letter) in examples(lang, site) {
            let html = letter.html(site, "anne@example.ch");
            let frame = |dark: bool| {
                format!(r#"<iframe title="{name} {mode}" srcdoc="{doc}"></iframe>"#, mode = if dark { "dark" } else { "light" }, doc = crate::site::esc(&crate::letter::standalone(&html, dark)))
            };
            sections += &format!(
                r#"<section><h2>{code} · {name}</h2><p class="subject">{subject}</p><div class="pair">{light}{dark}</div><details><summary>Plain text</summary><pre>{text}</pre></details></section>"#,
                code = lang.code().to_uppercase(),
                subject = crate::site::esc(&letter.subject()),
                light = frame(false),
                dark = frame(true),
                text = crate::site::esc(&letter.text(site)),
            );
        }
    }
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>EquiEvents e-mails</title><style>
body{{margin:0;padding:24px 16px;background:#e9edf2;font:14px/1.5 -apple-system,system-ui,sans-serif;color:#0d1b2a}}
h1{{margin:0 0 4px;font-size:22px}} h2{{margin:0;font-size:15px}} .subject{{margin:2px 0 10px;color:#3a4859}}
section{{max-width:1240px;margin:0 auto 32px}}
.pair{{display:grid;grid-template-columns:repeat(auto-fit,minmax(320px,1fr));gap:16px}}
iframe{{width:100%;height:620px;border:1px solid #c9d2dc;border-radius:10px;background:#fff}}
pre{{white-space:pre-wrap;background:#fff;padding:12px;border-radius:8px}}
</style></head><body><section><h1>EquiEvents e-mails</h1><p>Every e-mail equievents.ch sends, in French, German and Italian: light on the left, dark on the right (as Apple Mail shows it to a reader in dark mode). Narrow the window to see them on a phone.</p></section>{sections}</body></html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SITE: &str = "https://www.equievents.ch";

    #[test]
    fn worded() {
        let letter = request_made(Lang::De, "Anne", "Bruno", "bruno@example.ch", "CSN Chalet-à-Gobet", true);
        assert_eq!(letter.subject(), "EquiEvents: Übernahmeanfrage für CSN Chalet-à-Gobet");
        let body = letter.text(SITE);
        assert!(body.starts_with("Guten Tag Anne\n") && body.contains("Bruno (bruno@example.ch)") && body.contains("EquiEvents öffnen: https://www.equievents.ch/app/"));
        assert_eq!(request_answered(Lang::Fr, "Bruno", "CSN", false, false).subject(), "EquiEvents : demande pour CSN refusée");
        assert!(access_given(Lang::Fr, "Dora", "Anne", "CSN").text(SITE).contains("Anne vous a donné accès au concours CSN"));
        let letter = request_made(Lang::It, "Anna", "Bruno", "bruno@example.ch", "CSN Lugano", false);
        assert!(letter.subject() == "EquiEvents: richiesta di accesso a CSN Lugano" && letter.text(SITE).starts_with("Buongiorno Anna,\n"));
        assert!(owner_replaced(Lang::Fr, "Anne", "Bruno", "CSN", true).text(SITE).contains("a nommé Bruno responsable du concours CSN. Vous y restez comme collaborateur·trice."));
        // Every one, in every language: a title, a greeting, the reader's address at the bottom.
        for lang in Lang::ALL {
            for (name, letter) in examples(lang, SITE) {
                let html = letter.html(SITE, "anne@example.ch");
                assert!(html.contains("<h1") && html.contains("anne@example.ch") && html.contains(&format!(r#"<html lang="{}">"#, lang.code())), "{name}");
            }
        }
        assert_eq!(preview(SITE).matches("<iframe").count(), 2 * 3 * examples(Lang::Fr, SITE).len());
    }
}
