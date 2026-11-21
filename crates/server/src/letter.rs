//! The e-mails' form (the user, 2026-11-21: HTML e-mails, light and dark as the reader's mail app
//! is). Each e-mail is written once as a [`Letter`] (its topic, the greeting, paragraphs, a button,
//! the small print) and goes out as plain text and HTML together (`multipart/alternative`, the
//! mark as an inline image: `mail`), so a mail app that shows no HTML shows the text.
//!
//! The HTML is the site's look: the navy band with the mark, the card, the coral button; tables
//! and inline styles, as mail apps want them; one column, 600 px at most (phones: the whole
//! width). Light or dark: Apple Mail and Outlook for Mac follow the reader's setting
//! (`prefers-color-scheme`), Outlook on the web too (`[data-ogsc]`, `[data-ogsb]`); Gmail and
//! Outlook on Windows darken e-mails their own way, and the colours are chosen to stay readable
//! then (the band and the button are dark or strong in both).

use crate::lang::Lang;
use crate::site::esc;

/// The mark (`app/src/ui/Logo.tsx`), 96 px for 32 px shown: sharp on phones.
pub const MARK: &[u8] = include_bytes!("../assets/mail-mark.png");
/// Its `Content-ID` in the e-mail (`cid:` in the HTML).
pub const MARK_ID: &str = "mark@equievents.ch";

/// The button under the paragraphs.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    None,
    /// EquiEvents online (`/app/`): where the reader acts on it.
    OpenApp,
    /// A link of its own (a password reset).
    Link { label: String, url: String },
}

/// One e-mail, in the reader's language.
#[derive(Debug, Clone, PartialEq)]
pub struct Letter {
    pub lang: Lang,
    /// What it is about, after « EquiEvents : » in the subject (the card's title, capitalised).
    pub topic: String,
    /// Whom it greets.
    pub name: String,
    pub paragraphs: Vec<String>,
    pub action: Action,
    /// Small print, after the button.
    pub after: Vec<String>,
}

const FONT: &str = "Inter,-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif";

/// Type as inline CSS, in longhand (Outlook on Windows reads no shorthand for it).
fn type_css(size: u32, line: &str, weight: u32, color: &str) -> String {
    format!("font-family:{FONT};font-size:{size}px;line-height:{line};font-weight:{weight};color:{color}")
}

impl Letter {
    pub fn new(lang: Lang, topic: impl Into<String>, name: &str, paragraphs: Vec<String>) -> Letter {
        Letter { lang, topic: topic.into(), name: name.to_string(), paragraphs, action: Action::None, after: Vec::new() }
    }

    pub fn open_app(mut self) -> Letter {
        self.action = Action::OpenApp;
        self
    }

    pub fn link(mut self, label: &str, url: String) -> Letter {
        self.action = Action::Link { label: label.to_string(), url };
        self
    }

    pub fn after(mut self, text: impl Into<String>) -> Letter {
        self.after.push(text.into());
        self
    }

    pub fn subject(&self) -> String {
        format!("EquiEvents{} {}", self.lang.pick(" :", ":", ":"), self.topic)
    }

    fn greeting(&self) -> String {
        match self.lang {
            Lang::Fr => format!("Bonjour {},", self.name),
            Lang::De => format!("Guten Tag {}", self.name),
            Lang::It => format!("Buongiorno {},", self.name),
        }
    }

    /// The button: its words and where it goes (`site`: `APP_URL`).
    fn button(&self, site: &str) -> Option<(String, String)> {
        match &self.action {
            Action::None => None,
            Action::OpenApp => Some((self.lang.pick("Ouvrir EquiEvents", "EquiEvents öffnen", "Apri EquiEvents").to_string(), format!("{}/app/", site.trim_end_matches('/')))),
            Action::Link { label, url } => Some((label.clone(), url.clone())),
        }
    }

    /// The plain text.
    pub fn text(&self, site: &str) -> String {
        let mut parts = vec![self.greeting()];
        parts.extend(self.paragraphs.iter().cloned());
        match (&self.action, self.button(site)) {
            (Action::Link { .. }, Some((_, url))) => parts.push(url),
            (_, Some((label, url))) => parts.push(format!("{label}{} {url}", self.lang.pick(" :", ":", ":"))),
            _ => {}
        }
        parts.extend(self.after.iter().cloned());
        parts.push("EquiEvents".into());
        parts.join("\n\n") + "\n"
    }

    /// The HTML (`to`: the address it goes to, said at the bottom).
    pub fn html(&self, site: &str, to: &str) -> String {
        let lang = self.lang;
        let site = site.trim_end_matches('/');
        let title = {
            let mut c = self.topic.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        };
        let (text, small) = (type_css(15, "1.6", 400, "#3a4859"), type_css(13, "1.5", 400, "#66768a"));
        let p = |words: &str, class: &str, style: &str| format!(r#"<p class="{class}" style="margin:0 0 14px;{style}">{}</p>"#, esc(words));
        let mut body = String::new();
        body += &format!(r#"<h1 class="ink" style="margin:0 0 18px;letter-spacing:-0.01em;{}">{}</h1>"#, type_css(23, "1.25", 700, "#0d1b2a"), esc(&title));
        body += &p(&self.greeting(), "ink2", &text);
        for paragraph in &self.paragraphs {
            body += &p(paragraph, "ink2", &text);
        }
        if let Some((label, url)) = self.button(site) {
            body += &format!(
                r##"<table role="presentation" cellpadding="0" cellspacing="0" border="0" style="margin:22px 0 18px"><tr><td class="button" bgcolor="#e5484d" style="border-radius:9px;background:#e5484d"><a href="{url}" target="_blank" style="display:inline-block;padding:13px 22px;border-radius:9px;{button};text-decoration:none">{label}</a></td></tr></table>"##,
                url = esc(&url),
                label = esc(&label),
                button = type_css(15, "1.2", 600, "#ffffff"),
            );
            if matches!(self.action, Action::Link { .. }) {
                body += &format!(
                    r#"<p class="ink3" style="margin:0 0 14px;{small}">{} <a class="link" href="{url}" style="color:#cc3a3f;word-break:break-all">{url}</a></p>"#,
                    esc(lang.pick("Le bouton ne fonctionne pas ? Ouvrez ce lien :", "Der Knopf funktioniert nicht? Öffnen Sie diesen Link:", "Il pulsante non funziona? Aprite questo link:")),
                    url = esc(&url),
                );
            }
        }
        for small_print in &self.after {
            body += &p(small_print, "ink3", &small);
        }
        let reason = match lang {
            Lang::Fr => format!("Cet e-mail a été envoyé à {} pour votre compte EquiEvents.", esc(to)),
            Lang::De => format!("Diese E-Mail ging an {}, für Ihr EquiEvents-Konto.", esc(to)),
            Lang::It => format!("Questa e-mail è stata inviata a {} per il vostro account EquiEvents.", esc(to)),
        };
        // What the mail app shows beside the subject: the first paragraph.
        let preview = esc(self.paragraphs.first().map(String::as_str).unwrap_or(""));
        format!(
            r##"<!doctype html>
<html lang="{code}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="x-apple-disable-message-reformatting">
<meta name="color-scheme" content="light dark">
<meta name="supported-color-schemes" content="light dark">
<title>{subject}</title>
<style>
:root{{color-scheme:light dark;supported-color-schemes:light dark}}
body{{margin:0;padding:0;width:100%;-webkit-text-size-adjust:100%;-ms-text-size-adjust:100%}}
table{{border-collapse:collapse}}
img{{border:0;outline:none;text-decoration:none}}
.frame{{border-collapse:separate!important;border-radius:16px;box-shadow:0 1px 2px rgba(16,32,48,.06),0 14px 34px -16px rgba(16,32,48,.28)}}
.band{{background-image:radial-gradient(80% 120% at 100% 0%,rgba(242,90,93,.24),rgba(242,90,93,0) 55%),radial-gradient(60% 90% at 0% 100%,rgba(31,111,196,.2),rgba(31,111,196,0) 60%)}}
@media (max-width:620px){{.card{{padding:26px 20px 18px!important}}.band{{padding:18px 20px!important}}.outer{{padding:12px 8px 24px!important}}}}
@media (prefers-color-scheme:dark){{
.page{{background:#03080d!important}}
.frame{{box-shadow:0 0 0 1px rgba(120,160,200,.07),0 24px 48px -20px rgba(0,0,0,.8)}}
.band{{background-color:#0e3048!important;background-image:radial-gradient(80% 140% at 100% 0%,rgba(242,90,93,.34),rgba(242,90,93,0) 58%),radial-gradient(70% 110% at 0% 100%,rgba(60,140,230,.3),rgba(60,140,230,0) 62%)!important;border:1px solid #24506f!important;border-bottom:0!important}}
.card{{background:#0f2132!important;border-color:#24506f!important}}
.ink{{color:#e9eff6!important}}
.ink2{{color:#b8c6d4!important}}
.ink3{{color:#8195a9!important}}
.foot{{color:#5d7186!important}}
.button{{background:#ef5559!important}}
.link{{color:#ff7477!important}}
}}
[data-ogsc] .ink{{color:#e9eff6!important}}
[data-ogsc] .ink2{{color:#b8c6d4!important}}
[data-ogsc] .ink3{{color:#8195a9!important}}
[data-ogsc] .link{{color:#ff7477!important}}
[data-ogsb] .page{{background:#03080d!important}}
[data-ogsb] .band{{background-color:#0e3048!important}}
[data-ogsb] .card{{background:#0f2132!important}}
</style>
</head>
<body class="page" style="margin:0;padding:0;background:#f3f5f8">
<div style="display:none;max-height:0;max-width:0;overflow:hidden;opacity:0;mso-hide:all">{preview}&#8199;&#847;&#8199;&#847;&#8199;&#847;&#8199;&#847;&#8199;&#847;&#8199;&#847;</div>
<table role="presentation" class="page" width="100%" cellpadding="0" cellspacing="0" border="0" bgcolor="#f3f5f8" style="background:#f3f5f8">
<tr><td class="outer" align="center" style="padding:28px 12px 36px">
<table role="presentation" class="frame" width="100%" cellpadding="0" cellspacing="0" border="0" style="max-width:600px;width:100%">
<tr><td class="band" bgcolor="#00283c" style="background-color:#00283c;border-radius:16px 16px 0 0;padding:20px 32px">
<table role="presentation" cellpadding="0" cellspacing="0" border="0"><tr>
<td style="padding:0 10px 0 0;vertical-align:middle"><img src="cid:{MARK_ID}" width="32" height="32" alt="" style="display:block;width:32px;height:32px;border-radius:9px"></td>
<td style="vertical-align:middle;letter-spacing:-0.01em;{wordmark}">Equi<span style="color:#ef5559">Events</span></td>
</tr></table>
</td></tr>
<tr><td class="card" bgcolor="#ffffff" style="background:#ffffff;border:1px solid #e4e9f0;border-top:0;border-radius:0 0 16px 16px;padding:32px 32px 20px">
{body}
</td></tr>
</table>
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="max-width:600px;width:100%">
<tr><td class="foot" style="padding:18px 8px 0;text-align:center;{foot}">{reason}<br><a class="foot" href="{site}" style="color:#98a5b5;text-decoration:underline">equievents.ch</a></td></tr>
</table>
</td></tr>
</table>
</body>
</html>
"##,
            code = lang.code(),
            subject = esc(&self.subject()),
            wordmark = type_css(18, "1", 700, "#ffffff"),
            foot = type_css(12, "1.5", 400, "#98a5b5"),
        )
    }
}

/// The HTML as a browser shows it on its own (the preview): the mark in the page itself, not
/// beside it in the e-mail; `dark`: dark whatever the system is.
pub fn standalone(html: &str, dark: bool) -> String {
    let html = html.replace(&format!("cid:{MARK_ID}"), &format!("data:image/png;base64,{}", base64(MARK)));
    if dark {
        html.replace("@media (prefers-color-scheme:dark)", "@media all")
    } else {
        html.replace("@media (prefers-color-scheme:dark)", "@media not all")
    }
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= chunk.len() { ABC[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_and_html() {
        let letter = Letter::new(Lang::Fr, "accès à CSN <Test>", "Anne & Co", vec!["Bruno vous a donné accès.".into()]).open_app().after("Petit texte.");
        assert_eq!(letter.subject(), "EquiEvents : accès à CSN <Test>");
        assert_eq!(letter.text("https://www.equievents.ch/"), "Bonjour Anne & Co,\n\nBruno vous a donné accès.\n\nOuvrir EquiEvents : https://www.equievents.ch/app/\n\nPetit texte.\n\nEquiEvents\n");
        let html = letter.html("https://www.equievents.ch/", "anne@example.ch");
        assert!(html.contains("<h1") && html.contains("Accès à CSN &lt;Test&gt;") && html.contains("Bonjour Anne &amp; Co,"));
        assert!(html.contains(r#"href="https://www.equievents.ch/app/""#) && html.contains("cid:mark@equievents.ch") && html.contains("anne@example.ch"));
        assert!(html.contains("prefers-color-scheme:dark") && html.contains(r#"<html lang="fr">"#));
        let link = Letter::new(Lang::De, "neues Passwort", "Anne", vec!["Öffnen Sie diesen Link:".into()]).link("Neues Passwort wählen", "https://x.ch/neues-passwort?t=a&b".into());
        assert!(link.text("https://x.ch").contains("Öffnen Sie diesen Link:\n\nhttps://x.ch/neues-passwort?t=a&b\n\nEquiEvents"));
        assert!(link.html("https://x.ch", "a@x.ch").contains(r#"href="https://x.ch/neues-passwort?t=a&amp;b""#));
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert!(standalone(&link.html("https://x.ch", "a@x.ch"), true).contains("data:image/png;base64,iVBOR"));
    }
}
