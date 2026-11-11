//! Credential redaction: the only code that knows the shape of a secret in text.
//!
//! `redact_url_userinfo` redacts URL credentials (the Self run's journal and reply,
//! the Persona's structured output). `scrub` is chat ingest's door: every message body
//! passes it before it is chunked, embedded or stored, and it also runs the URL rule.
//! Shapes only; nothing here reads meaning, and a hit is a kind, never a value.

/// The one redaction of URL credentials. Call it at each place a string the CLI produced enters the journal, her mind or chat: today
/// the Self result (`result_text`: journal line and Reply) and the structured output of a Persona or Sleep result
/// (`persona::structured_output`, string by string). It does not cover claude's stderr (inherited) or the CLI's transcript files.
///
/// A forge token rides in a remote URL, so every `scheme://` and every protocol-relative `//` opens an authority, read as git, curl and
/// RFC 3986 read it: it ends at the first `/`, `?`, `#`, backslash, quote, backtick, parenthesis, bracket, angle bracket or ASCII whitespace
/// (`is_authority_end`). Its userinfo is everything before the authority's LAST `@` (a raw `@` in a password still redacts), and non-ASCII
/// characters, NBSP and U+2028 included, stay inside it. The userinfo becomes `<redacted>`: `scheme://<redacted>@host`. With a colon it is
/// always redacted; without one it is a bare token on http, https, ftp, ftps, ws, wss, any `+http`/`+https` scheme (`git+https`), protocol-relative `//` and an unparseable scheme, and a plain
/// username on any other scheme (`ssh://git@host`), which stays. A `@` after the authority (`github.com/@user`, `pkg@1.2.3`) is path, not
/// userinfo, and is left alone. Scheme-less credentials are covered only in the shape `word:secret@host.tld/` at the start of an
/// ASCII-whitespace-delimited word. Not covered: a password holding `/ ? # ` a quote or ASCII whitespace (no parser reads it as userinfo),
/// scp-style `user:secret@host:path`, tokens in query strings, claude's stderr, the kept transcripts. Syntax only; no reading of meaning.
pub fn redact_url_userinfo(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("//") {
        let (before, after) = rest.split_at(at);
        let body = &after[2..];
        out.push_str(before);
        out.push_str("//");
        // The scheme is the ASCII run before `:`. Without a colon, `//` opens an authority only at a word start (`a//b@c` is not one).
        let scheme = match before.strip_suffix(':') {
            // Only ASCII bytes match, so the byte after the last non-match is a character boundary.
            Some(head) => head[head.bytes().rposition(|c| !(c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.'))).map_or(0, |i| i + 1)..].to_ascii_lowercase(),
            None if before.chars().next_back().is_none_or(|c| is_break(c) || matches!(c, '"' | '\'' | '`' | '(' | '[' | '{' | '<' | '=' | ',')) => String::new(),
            None => {
                rest = body;
                continue;
            }
        };
        let end = body.find(is_authority_end).unwrap_or(body.len());
        let authority = &body[..end];
        match authority.rfind('@') {
            Some(i) if i > 0 && (authority[..i].contains(':') || carries_bare_token(&scheme)) => {
                out.push_str("<redacted>");
                out.push_str(&authority[i..]);
            }
            _ => out.push_str(authority),
        }
        rest = &body[end..];
    }
    out.push_str(rest);
    redact_schemeless(&out)
}

/// Schemes whose lone userinfo is a token, not a username: web and ftp schemes, `ws`/`wss`, and `x+http`/`x+https` wrappers; `scheme` is lowercase.
fn carries_bare_token(scheme: &str) -> bool {
    matches!(scheme, "" | "http" | "https" | "ftp" | "ftps" | "ws" | "wss") || scheme.ends_with("+http") || scheme.ends_with("+https")
}

/// ASCII whitespace; NBSP, U+2028 and other Unicode whitespace are not, so a secret holding them is redacted whole.
fn is_break(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c')
}

/// The end of a URL's authority.
fn is_authority_end(c: char) -> bool {
    is_break(c) || matches!(c, '/' | '?' | '#' | '\\' | '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>')
}

/// `word:secret@host.tld/` at the start of a whitespace-delimited word that holds no `//` becomes `<redacted>@host.tld/`.
fn redact_schemeless(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        out.push_str(&redact_word(word));
        word.clear();
    };
    for c in text.chars() {
        if is_break(c) {
            flush(&mut word, &mut out);
            out.push(c);
        } else {
            word.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

fn redact_word(word: &str) -> String {
    let shaped = (|| {
        if word.contains("//") {
            return None;
        }
        let at = word.rfind('@')?;
        let colon = word.find(':')?;
        let (user, host) = (&word[..colon], &word[at + 1..]);
        let host_name = &host[..host.find('/')?];
        let user_ok = !user.is_empty() && user.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        let host_ok = host_name.contains('.') && host_name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
        (colon + 1 < at && user_ok && host_ok).then_some(at)
    })();
    match shaped {
        Some(at) => format!("<redacted>{}", &word[at..]),
        None => word.to_string(),
    }
}

/// The version of the scrub's rules. Part of the chat version: a bump makes every
/// room re-ingest from the room's history and scrub it again.
pub const SCRUB_VERSION: u32 = 1;

/// What replaces a secret.
pub const REDACTED: &str = "<redacted>";

/// The shape of secret a scrub found. Never its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScrubKind {
    UrlUserinfo,
    Bearer,
    AuthorizationHeader,
    KeyShape,
    Jwt,
    PemBlock,
}

impl std::fmt::Display for ScrubKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UrlUserinfo => "url-userinfo",
            Self::Bearer => "bearer",
            Self::AuthorizationHeader => "authorization-header",
            Self::KeyShape => "key-shape",
            Self::Jwt => "jwt",
            Self::PemBlock => "pem-block",
        })
    }
}

/// A body with its secrets replaced by `<redacted>`, and the kinds found, one per secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scrubbed {
    pub text: String,
    pub hits: Vec<ScrubKind>,
}

/// Replaces credential shapes in `text`: a PEM private key block, an `authorization:` header line,
/// `Bearer`/`Basic`/`token` followed by a long token, a vendor key shape, a JWT, and URL userinfo.
/// Each hit becomes `<redacted>`. Token shapes only; ordinary prose, SHAs, UUIDs and ids pass byte for byte.
pub fn scrub(text: &str) -> Scrubbed {
    let mut hits = Vec::new();
    let mut text = text.to_owned();
    let passes: [(ScrubKind, fn(&str, usize) -> Option<usize>); 5] = [
        (ScrubKind::PemBlock, pem_block),
        (ScrubKind::AuthorizationHeader, authorization_line),
        (ScrubKind::Bearer, bearer_token),
        (ScrubKind::KeyShape, key_shape),
        (ScrubKind::Jwt, jwt),
    ];
    for (kind, find) in passes {
        text = replace_matches(&text, kind, find, &mut hits);
    }
    let urls = redact_url_userinfo(&text);
    let (before, after) = (text.matches(REDACTED).count(), urls.matches(REDACTED).count());
    hits.extend(std::iter::repeat_n(ScrubKind::UrlUserinfo, after.saturating_sub(before)));
    Scrubbed { text: urls, hits }
}

fn replace_matches(text: &str, kind: ScrubKind, find: fn(&str, usize) -> Option<usize>, hits: &mut Vec<ScrubKind>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while at < text.len() {
        match find(text, at) {
            Some(end) if end > at => {
                out.push_str(REDACTED);
                hits.push(kind);
                at = end;
            }
            _ => {
                let next = at + text[at..].chars().next().map_or(1, char::len_utf8);
                out.push_str(&text[at..next]);
                at = next;
            }
        }
    }
    out
}

/// Whether `at` starts a word: the byte before it is not an ASCII letter or digit.
fn word_start(text: &str, at: usize) -> bool {
    at == 0 || !text.as_bytes()[at - 1].is_ascii_alphanumeric()
}

/// The end of the run of bytes at `from` that `member` accepts.
fn run_end(text: &str, from: usize, member: impl Fn(u8) -> bool) -> usize {
    from + text.as_bytes()[from..].iter().take_while(|byte| member(**byte)).count()
}

fn starts_with_ignore_case(text: &str, at: usize, word: &str) -> bool {
    text.as_bytes().get(at..at + word.len()).is_some_and(|held| held.eq_ignore_ascii_case(word.as_bytes()))
}

fn token_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'~' | b'+' | b'/' | b'=' | b'-')
}

fn key_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

/// `-----BEGIN ... PRIVATE KEY-----` through its `-----END ... PRIVATE KEY-----`, or the text's end.
fn pem_block(text: &str, at: usize) -> Option<usize> {
    let rest = &text[at..];
    let label = rest.strip_prefix("-----BEGIN ")?;
    let words = run_end(label, 0, |byte| byte.is_ascii_uppercase() || byte == b' ');
    label[..words].ends_with("PRIVATE KEY").then_some(())?;
    label[words..].starts_with("-----").then_some(())?;
    let body_from = at + "-----BEGIN ".len() + words + 5;
    let end = text[body_from..]
        .find("-----END ")
        .and_then(|end| text[body_from + end..].find("PRIVATE KEY-----").map(|tail| body_from + end + tail + "PRIVATE KEY-----".len()));
    Some(end.unwrap_or(text.len()))
}

/// `authorization:` in any case at a word start, through the end of the line.
fn authorization_line(text: &str, at: usize) -> Option<usize> {
    (word_start(text, at) && starts_with_ignore_case(text, at, "authorization:")).then_some(())?;
    let from = at + "authorization:".len();
    let end = run_end(text, from, |byte| byte != b'\n' && byte != b'\r');
    (!text[from..end].trim().is_empty()).then_some(end)
}

/// `Bearer`, `Basic` or `token`, blanks, then 16 or more token characters.
fn bearer_token(text: &str, at: usize) -> Option<usize> {
    word_start(text, at).then_some(())?;
    let word = ["bearer", "basic", "token"].into_iter().find(|word| starts_with_ignore_case(text, at, word))?;
    let blanks = at + word.len();
    let value = run_end(text, blanks, |byte| byte == b' ' || byte == b'\t');
    (value > blanks).then_some(())?;
    let end = run_end(text, value, token_char);
    (end - value >= 16).then_some(end)
}

/// A vendor key at a word start: the prefix, then a long enough run of the key alphabet.
fn key_shape(text: &str, at: usize) -> Option<usize> {
    word_start(text, at).then_some(())?;
    let rest = &text[at..];
    let by_prefix = [("sk-", 16), ("sk_live_", 16), ("pk_live_", 16), ("github_pat_", 16), ("glpat-", 16)];
    let tokens = ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"].into_iter().map(|prefix| (prefix, 16));
    for (prefix, least) in by_prefix.into_iter().chain(tokens) {
        if rest.starts_with(prefix) {
            let end = run_end(text, at + prefix.len(), key_char);
            return (end - at - prefix.len() >= least).then_some(end);
        }
    }
    for prefix in ["xoxa-", "xoxb-", "xoxp-", "xoxr-", "xoxs-"] {
        if rest.starts_with(prefix) {
            let end = run_end(text, at + prefix.len(), |byte| byte.is_ascii_alphanumeric() || byte == b'-');
            return (end - at - prefix.len() >= 16).then_some(end);
        }
    }
    for prefix in ["AKIA", "ASIA"] {
        if rest.starts_with(prefix) {
            let end = run_end(text, at + prefix.len(), |byte| byte.is_ascii_uppercase() || byte.is_ascii_digit());
            return (end - at - prefix.len() >= 16).then_some(end);
        }
    }
    if rest.starts_with("AIza") {
        let end = run_end(text, at + 4, key_char);
        return (end - at - 4 >= 35).then_some(end);
    }
    None
}

/// Three base64url segments, the first starting `eyJ`.
fn jwt(text: &str, at: usize) -> Option<usize> {
    (word_start(text, at) && text[at..].starts_with("eyJ")).then_some(())?;
    let segment = |from: usize| run_end(text, from, key_char);
    let first = segment(at);
    let mut end = first;
    for _ in 0..2 {
        (text.as_bytes().get(end) == Some(&b'.')).then_some(())?;
        let next = segment(end + 1);
        (next > end + 1).then_some(())?;
        end = next;
    }
    Some(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANARY: &str = "CANARY-NOT-A-REAL-TOKEN-0000";

    #[test]
    fn url_userinfo_is_redacted_where_it_is_a_secret_and_kept_where_it_is_a_username() {
        let said = format!(
            "a https://user:{CANARY}@host.example/r.git b https://{CANARY}@host.example/r.git c ssh://u:{CANARY}@h/x d http://{CANARY}@h e HTTPS://x:{CANARY}@h \
             f https://u:p@ss{CANARY}@h/x g ssh://git@host.example/r.git h git://git@h/x i https://host.example/no-userinfo j (https://u:{CANARY}@h) k ://@x"
        );
        let want = "a https://<redacted>@host.example/r.git b https://<redacted>@host.example/r.git c ssh://<redacted>@h/x d http://<redacted>@h e HTTPS://<redacted>@h \
             f https://<redacted>@h/x g ssh://git@host.example/r.git h git://git@h/x i https://host.example/no-userinfo j (https://<redacted>@h) k ://@x";
        assert_eq!(redact_url_userinfo(&said), want);
        assert!(!redact_url_userinfo(&said).contains(CANARY));
    }

    #[test]
    fn a_secret_is_redacted_whole_by_the_authority_rule_and_commonplace_urls_are_left_alone() {
        let r = redact_url_userinfo;
        // Raw @, percent-encoding and non-ASCII (NBSP, U+2028) stay inside userinfo.
        for secret in ["pa@ssT", "pa%40ssT", "pa\u{a0}ssT", "pa\u{2028}ssT", "pa:ssT", "\u{e9}\u{65e5}ssT"] {
            let out = r(&format!("x https://u:{secret}@h/x"));
            assert_eq!(out, "x https://<redacted>@h/x", "{secret:?}");
        }
        // The authority ends at / ? # \\ quotes, brackets and ASCII whitespace: no parser reads those as userinfo, so what follows is path.
        assert_eq!(r("https://u:pa/ssT@h/x"), "https://u:pa/ssT@h/x");
        assert_eq!(r("https://u:pa ssT@h/x"), "https://u:pa ssT@h/x");
        assert_eq!(r("https://u:pa?ssT@h"), "https://u:pa?ssT@h");
        // Ordinary URLs with @ in the path are byte-identical.
        for same in [
            "https://github.com/@user/repo",
            "https://medium.com/@name/post-1",
            "https://registry.npmjs.org/@scope/pkg",
            "https://h/pkg@1.2.3",
            "a//b@c",
            "see //cdn.h/@x/y and https://h/?mail=a@b.c#f@g",
            "https://h:8080/p git@github.com:org/r.git ssh://git@host/r.git ssh://git@h:22/r git://git@h/x",
        ] {
            assert_eq!(r(same), same);
        }
        // Compact JSON: only the credentialed URL changes.
        let json = format!(r#"{{"a":"https://u:{CANARY}@h/x","b":"https://h2/@v","c":"https://h3/p@1"}}"#);
        assert_eq!(r(&json), r#"{"a":"https://<redacted>@h/x","b":"https://h2/@v","c":"https://h3/p@1"}"#);
        assert_eq!(r("a/https://u:T@h/x,https://v:T@h2"), "a/https://<redacted>@h/x,https://<redacted>@h2");
        assert_eq!(r("HTTPS://TOKEN@h"), "HTTPS://<redacted>@h");
        assert_eq!(r("[l](https://T@h/x) <https://T@h> `https://u:T@h`"), "[l](https://<redacted>@h/x) <https://<redacted>@h> `https://<redacted>@h`");
        assert_eq!(r("a //u:T@host/x b //T@host c \"//T@h\" (//T@h)"), "a //<redacted>@host/x b //<redacted>@host c \"//<redacted>@h\" (//<redacted>@h)");
        // Token-bearing schemes beyond http: +http/+https wrappers and ws/wss redact a bare token; git+ssh and ssh keep their username.
        assert_eq!(r("git+https://T@github.com/a/b GIT+HTTPS://T@h wss://T@h/ ws://T@h/"), "git+https://<redacted>@github.com/a/b GIT+HTTPS://<redacted>@h wss://<redacted>@h/ ws://<redacted>@h/");
        for same in ["git+ssh://git@host/x", "ssh://git@host/x"] {
            assert_eq!(r(same), same);
        }
        // Scheme-less: only word:secret@host.tld/ . Each guard of the shape has its own case.
        assert_eq!(r("user:T@host.tld/path"), "<redacted>@host.tld/path");
        for same in ["a:b@c", "a:b@host/x", "a:b@ho_st.tld/x", "u$er:T@host.tld/x", "a:@host.tld/x", "mail me@x.org/ and :T@host.tld/x"] {
            assert_eq!(r(same), same);
        }
    }

    #[test]
    fn multibyte_text_never_panics_the_redaction() {
        let r = redact_url_userinfo;
        for case in ["see \u{201c}https://h/x", "\u{2192}https://h/x", "docs\u{ff1a}https://h/x", "\u{e9}://x", "\u{65e5}\u{672c}://u:T@h", "https://x/\u{e9}://", "\u{e9}//u:T@h", "\u{e9}:u:T@h.tld/"] {
            let _ = r(case);
        }
        assert_eq!(r("\u{65e5}\u{672c}://u:T@h"), "\u{65e5}\u{672c}://<redacted>@h");
        assert_eq!(r("\u{2192}https://u:T@h/x"), "\u{2192}https://<redacted>@h/x");
        let seed = "\u{201c}\u{2192}\u{ff1a}\u{e9}\u{65e5}\u{a0}https://u:T@h/x//y:z@a.b/ ://@ //";
        let chars: Vec<char> = seed.chars().collect();
        for start in 0..=chars.len() {
            for end in start..=chars.len() {
                let _ = r(&chars[start..end].iter().collect::<String>());
            }
        }
    }


    /// Every shape is built here from fake parts; none is a real credential.
    fn secrets() -> Vec<(ScrubKind, String, String)> {
        let pem = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEAfakefakefake\n-----END RSA PRIVATE KEY-----".to_owned();
        vec![
            (ScrubKind::AuthorizationHeader, format!("Authorization: Bearer {}", "a1B2".repeat(8)), "a1B2a1B2".into()),
            (ScrubKind::Bearer, format!("Bearer {}", "z9Y8x7".repeat(4)), "z9Y8x7z9Y8".into()),
            (ScrubKind::Bearer, format!("Basic {}", "dXNlcjpwYXNz".repeat(2)), "dXNlcjpwYXNzdXNl".into()),
            (ScrubKind::KeyShape, format!("sk-{}", "q7W3".repeat(8)), "q7W3q7W3".into()),
            (ScrubKind::KeyShape, format!("sk_live_{}", "m4N5".repeat(6)), "m4N5m4N5".into()),
            (ScrubKind::KeyShape, format!("ghp_{}", "k2L9".repeat(9)), "k2L9k2L9".into()),
            (ScrubKind::KeyShape, format!("github_pat_{}_{}", "A1b2C3".repeat(4) + "ab", "d4E5f6".repeat(9) + "abcde"), "A1b2C3A1b2".into()),
            (ScrubKind::KeyShape, format!("glpat-{}", "r6T7".repeat(5)), "r6T7r6T7".into()),
            (ScrubKind::KeyShape, "xoxb-123456789012-987654321098-aBcDeFgHiJkLmNoP".into(), "aBcDeFgHiJkL".into()),
            (ScrubKind::KeyShape, "AKIA0123456789ABCDEF".into(), "0123456789ABCDEF".into()),
            (ScrubKind::KeyShape, format!("AIza{}", "Sy1_".repeat(9)), "Sy1_Sy1_Sy1_".into()),
            (ScrubKind::Jwt, "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJmYWtlIn0.c2lnbmF0dXJlZmFrZQ".into(), "c2lnbmF0dXJlZmFrZQ".into()),
            (ScrubKind::PemBlock, pem, "MIIEowIBAAKCAQEAfake".into()),
            (ScrubKind::PemBlock, "-----BEGIN PRIVATE KEY-----\nMIIBVQIBADANfakeNOEND".into(), "MIIBVQIBADANfake".into()),
            (ScrubKind::UrlUserinfo, "https://user:hunter2hunter2@host.example/r.git".into(), "hunter2hunter2".into()),
            (ScrubKind::UrlUserinfo, "https://tokenvalue12345@host.example/".into(), "tokenvalue12345".into()),
        ]
    }

    #[test]
    fn every_planted_credential_is_replaced_mid_line_and_at_line_end_with_its_kind() {
        for (kind, secret, distinct) in secrets() {
            for body in [format!("before {secret} after"), format!("before {secret}"), format!("{secret}\nnext line")] {
                let out = scrub(&body);
                assert!(!out.text.contains(&distinct), "{kind}: {}", out.text);
                assert!(out.text.contains(REDACTED), "{kind}: {}", out.text);
                assert!(out.hits.contains(&kind), "{kind} not in {:?} for {body:?}", out.hits);
            }
        }
    }

    #[test]
    fn a_pem_block_without_its_end_runs_to_the_end_of_the_body_and_one_with_it_stops_there() {
        let open = scrub("see -----BEGIN EC PRIVATE KEY-----\nAAAA\nBBBB").text;
        assert_eq!(open, "see <redacted>");
        let closed = scrub("-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY----- and then prose").text;
        assert_eq!(closed, "<redacted> and then prose");
    }

    #[test]
    fn an_authorization_header_goes_to_the_end_of_its_line_only() {
        assert_eq!(scrub("hi\nAUTHORIZATION: Token abc\nbye").text, "hi\n<redacted>\nbye");
        assert_eq!(scrub("Authorization: x").hits, [ScrubKind::AuthorizationHeader]);
    }

    #[test]
    fn ordinary_text_passes_byte_for_byte_and_reports_no_hit() {
        for same in [
            "risk-averse", "task-force", "sk-", "sk-short", "3f2b8c1d9e7a4b5c6d0e1f2a3b4c5d6e7f8a9b0c",
            "123e4567-e89b-12d3-a456-426614174000", "!abc:s", "$Ab3dEf-9_zQ", "user@example.com", "ssh://git@host/r.git",
            "https://github.com/@user", "pkg@1.2.3", "the bearer of bad news", "Authorization is a header",
            "padding like dGVzdA== in prose", "```\nlet x = 1;\n```", "the BEGIN PRIVATE KEY header", "a token of thanks",
            "AKIAshort", "ghp_short", "eyJ is a prefix", "naïve café — 日本語 with Bearer in it",
        ] {
            let out = scrub(same);
            assert_eq!(out.text, same);
            assert!(out.hits.is_empty(), "{same}: {:?}", out.hits);
        }
    }

    #[test]
    fn multibyte_text_around_a_secret_never_panics_the_scrub() {
        let body = format!("日本 \u{a0}é Bearer {} → ünï", "A1b2".repeat(6));
        assert_eq!(scrub(&body).text, "日本 \u{a0}é <redacted> → ünï");
        let seed = "é日 sk-aaaaaaaaaaaaaaaaaaaa -----BEGIN PRIVATE KEY----- authorization: é eyJa.b.c https://u:p@h/x";
        let chars: Vec<char> = seed.chars().collect();
        for start in 0..chars.len() {
            for end in start..=chars.len() {
                let _ = scrub(&chars[start..end].iter().collect::<String>());
            }
        }
    }
}
