//! The library's public surface, pinned as a whole.
//!
//! The brake's rule is that no caller can name a root, a source or a way to make
//! a `Grant`. A test that names the routes it knows (`with_root`, `TEST_ROOT`)
//! passes on every route it does not name. This one reads the library's source
//! the way a consumer sees it and compares everything a consumer could reach to
//! `public_surface.txt`: every `pub` item with its signature, every `pub` field,
//! the derives and the header of every impl, every string literal that looks like
//! a path, and every `static` keyword (a process-global is the one channel
//! through which an existing public function's body could carry a root). A new
//! `pub static`, a new `pub fn` under any name, a `Clone` or `Default` derived
//! onto `Grant`, a root read from a file other than the constant: each is a line
//! this file does not have, and the test fails.
//!
//! Items under `#[cfg(test)]` are not the library and are skipped; `cfg(not(test))`
//! and the `unix`/`windows` splits are the library and are read. Any other `cfg`, a `#[path]`, an item macro, a
//! `macro_rules!`, an `extern` block or an exported symbol attribute would hide
//! an item from this reader, so each fails the test outright. A seam that is
//! `pub(crate)` and not `cfg(test)` is outside this reader's concern and inside
//! clippy's: it is dead code in the library build, and `-D warnings` refuses it.
//!
//! A deliberate change to the surface is a change to `public_surface.txt` in the
//! same commit, which is where a reviewer sees it. The failure prints the full
//! current surface to copy from.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
use quote::ToTokens;
use syn::{Attribute, Fields, ImplItem, Item, Visibility};

const SNAPSHOT: &str = include_str!("public_surface.txt");

/// Attributes that would put an item outside this reader's sight or export it by name.
const HIDING: [&str; 7] = ["path", "no_mangle", "export_name", "link_section", "macro_export", "macro_use", "cfg_attr"];

fn is_pub(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn without_docs(stream: TokenStream) -> TokenStream {
    let tokens: Vec<TokenTree> = stream.into_iter().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < tokens.len() {
        if matches!(&tokens[at], TokenTree::Punct(p) if p.as_char() == '#') {
            let mut group_at = at + 1;
            if matches!(tokens.get(group_at), Some(TokenTree::Punct(p)) if p.as_char() == '!') {
                group_at += 1;
            }
            if let Some(TokenTree::Group(group)) = tokens.get(group_at) {
                let is_doc = group.delimiter() == Delimiter::Bracket
                    && matches!(group.stream().into_iter().next(), Some(TokenTree::Ident(name)) if name == "doc");
                if is_doc {
                    at = group_at + 1;
                    continue;
                }
            }
        }
        match &tokens[at] {
            TokenTree::Group(group) => out.push(TokenTree::Group(Group::new(group.delimiter(), without_docs(group.stream())))),
            other => out.push(other.clone()),
        }
        at += 1;
    }
    out.into_iter().collect()
}

fn text(node: impl ToTokens) -> String {
    without_docs(node.to_token_stream()).to_string()
}

/// The string literals in `stream` that look like a path.
fn path_literals(stream: TokenStream, out: &mut BTreeSet<String>) {
    for token in stream {
        match token {
            TokenTree::Group(group) => path_literals(group.stream(), out),
            TokenTree::Literal(literal) => {
                let rendered = literal.to_string();
                if rendered.starts_with('"') && rendered.contains('/') {
                    out.insert(format!("path-literal {rendered}"));
                }
            }
            _ => {}
        }
    }
}

/// How many `static` items or declarations `stream` holds (a `'static` lifetime is not one).
/// A process-global is the one way an existing public function's body could carry a
/// root from one call to another, so every one in the library is in the snapshot.
fn static_keywords(stream: TokenStream) -> usize {
    let mut count = 0;
    let mut after_tick = false;
    for token in stream {
        let is_tick = matches!(&token, TokenTree::Punct(p) if p.as_char() == '\'');
        match token {
            TokenTree::Group(group) => count += static_keywords(group.stream()),
            TokenTree::Ident(name) if name == "static" && !after_tick => count += 1,
            _ => {}
        }
        after_tick = is_tick;
    }
    count
}

fn attributes(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(i) => &i.attrs,
        Item::Enum(i) => &i.attrs,
        Item::ExternCrate(i) => &i.attrs,
        Item::Fn(i) => &i.attrs,
        Item::ForeignMod(i) => &i.attrs,
        Item::Impl(i) => &i.attrs,
        Item::Macro(i) => &i.attrs,
        Item::Mod(i) => &i.attrs,
        Item::Static(i) => &i.attrs,
        Item::Struct(i) => &i.attrs,
        Item::Trait(i) => &i.attrs,
        Item::TraitAlias(i) => &i.attrs,
        Item::Type(i) => &i.attrs,
        Item::Union(i) => &i.attrs,
        Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

/// Whether the library build contains what these attributes sit on.
fn in_library(attrs: &[Attribute], at: &str) -> bool {
    let mut keep = true;
    for attr in attrs {
        let rendered = attr.to_token_stream().to_string().replace(' ', "");
        assert!(!HIDING.iter().any(|word| attr.path().is_ident(*word)) && !rendered.contains("no_mangle") && !rendered.contains("export_name"), "{at}: `{rendered}` hides an item from the surface test");
        if attr.path().is_ident("cfg") {
            match rendered.as_str() {
                "#[cfg(test)]" => keep = false,
                // Both sides of a platform split are the library, and both are read.
                "#[cfg(not(test))]" | "#[cfg(unix)]" | "#[cfg(not(unix))]" | "#[cfg(windows)]" | "#[cfg(not(windows))]" => {}
                other => panic!("{at}: `{other}`: the surface test reads only cfg(test), cfg(not(test)) and the unix/windows splits"),
            }
        }
    }
    keep
}

fn kept_fields(fields: &Fields) -> Vec<String> {
    match fields {
        Fields::Named(named) => named.named.iter().filter(|f| is_pub(&f.vis)).map(|f| format!("{}: {}", text(&f.ident), text(&f.ty))).collect(),
        Fields::Unnamed(unnamed) => unnamed.unnamed.iter().enumerate().filter(|(_, f)| is_pub(&f.vis)).map(|(i, f)| format!("{i}: {}", text(&f.ty))).collect(),
        Fields::Unit => Vec::new(),
    }
}

fn derives(attrs: &[Attribute]) -> String {
    attrs.iter().filter(|a| a.path().is_ident("derive") || a.path().is_ident("repr")).map(text).collect::<Vec<_>>().join(" ")
}

fn label(item: &Item) -> String {
    match item {
        Item::Fn(i) => i.sig.ident.to_string(),
        Item::Const(i) => i.ident.to_string(),
        Item::Static(i) => i.ident.to_string(),
        Item::Struct(i) => i.ident.to_string(),
        Item::Enum(i) => i.ident.to_string(),
        Item::Trait(i) => i.ident.to_string(),
        Item::Type(i) => i.ident.to_string(),
        _ => "an item".to_string(),
    }
}

fn walk(items: &[Item], path: &str, dir: &Path, out: &mut BTreeSet<String>) {
    for item in items {
        if !in_library(attributes(item), path) {
            continue;
        }
        if !matches!(item, Item::Impl(_) | Item::Mod(_)) {
            let stream = without_docs(item.to_token_stream());
            path_literals(stream.clone(), out);
            let statics = static_keywords(stream);
            if statics > 0 {
                out.insert(format!("{path}: {statics} static keyword(s) in {}", label(item)));
            }
        }
        match item {
            Item::Fn(f) if is_pub(&f.vis) => {
                out.insert(format!("{path}: {}", text(&f.sig)));
            }
            Item::Const(c) if is_pub(&c.vis) => {
                out.insert(format!("{path}: const {} : {}", c.ident, text(&c.ty)));
            }
            Item::Static(s) if is_pub(&s.vis) => {
                out.insert(format!("{path}: static {} : {} mutable={}", s.ident, text(&s.ty), matches!(s.mutability, syn::StaticMutability::Mut(_))));
            }
            Item::Struct(s) if is_pub(&s.vis) => {
                out.insert(format!("{path}: struct {}{} {} pub-fields[{}]", s.ident, text(&s.generics), derives(&s.attrs), kept_fields(&s.fields).join(", ")));
            }
            Item::Enum(e) if is_pub(&e.vis) => {
                out.insert(format!("{path}: {}", text(Item::Enum(e.clone()))));
            }
            Item::Union(u) if is_pub(&u.vis) => {
                out.insert(format!("{path}: {}", text(Item::Union(u.clone()))));
            }
            Item::Trait(t) if is_pub(&t.vis) => {
                out.insert(format!("{path}: {}", text(Item::Trait(t.clone()))));
            }
            Item::TraitAlias(t) if is_pub(&t.vis) => {
                out.insert(format!("{path}: {}", text(Item::TraitAlias(t.clone()))));
            }
            Item::Type(t) if is_pub(&t.vis) => {
                out.insert(format!("{path}: {}", text(Item::Type(t.clone()))));
            }
            Item::Use(u) if is_pub(&u.vis) => {
                out.insert(format!("{path}: {}", text(u)));
            }
            Item::Impl(i) => {
                let header = match &i.trait_ {
                    Some((bang, tr, _)) => format!("impl{} {}{} for {}", text(&i.generics), text(bang), text(tr), text(&i.self_ty)),
                    None => format!("impl{} {}", text(&i.generics), text(&i.self_ty)),
                };
                out.insert(format!("{path}: {header}"));
                path_literals(without_docs(i.generics.to_token_stream()), out);
                for member in &i.items {
                    let attrs = match member {
                        ImplItem::Const(m) => &m.attrs,
                        ImplItem::Fn(m) => &m.attrs,
                        ImplItem::Type(m) => &m.attrs,
                        ImplItem::Macro(m) => &m.attrs,
                        _ => &[][..],
                    };
                    if !in_library(attrs, path) {
                        continue;
                    }
                    let stream = without_docs(member.to_token_stream());
                    path_literals(stream.clone(), out);
                    let statics = static_keywords(stream);
                    if statics > 0 {
                        out.insert(format!("{path}: {statics} static keyword(s) in a member of impl {}", text(&i.self_ty)));
                    }
                    if i.trait_.is_none() {
                        match member {
                            ImplItem::Fn(f) if is_pub(&f.vis) => {
                                out.insert(format!("{path}: impl {}: {}", text(&i.self_ty), text(&f.sig)));
                            }
                            ImplItem::Const(c) if is_pub(&c.vis) => {
                                out.insert(format!("{path}: impl {}: const {} : {}", text(&i.self_ty), c.ident, text(&c.ty)));
                            }
                            ImplItem::Type(t) if is_pub(&t.vis) => {
                                out.insert(format!("{path}: impl {}: {}", text(&i.self_ty), text(t)));
                            }
                            ImplItem::Macro(_) | ImplItem::Verbatim(_) => panic!("{path}: an impl member the surface test cannot read"),
                            _ => {}
                        }
                    }
                }
            }
            Item::Mod(m) => {
                let name = m.ident.to_string();
                if is_pub(&m.vis) {
                    out.insert(format!("{path}: mod {name}"));
                }
                let inner = format!("{path}::{name}");
                let below = dir.join(&name);
                match &m.content {
                    Some((_, content)) => walk(content, &inner, &below, out),
                    None => {
                        let file = [dir.join(format!("{name}.rs")), below.join("mod.rs")].into_iter().find(|candidate| candidate.exists());
                        let file = file.unwrap_or_else(|| panic!("{inner}: no source file"));
                        let parsed = syn::parse_file(&fs::read_to_string(&file).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
                        if in_library(&parsed.attrs, &inner) {
                            walk(&parsed.items, &inner, &below, out);
                        }
                    }
                }
            }
            Item::Macro(_) => panic!("{path}: an item macro in library source: the surface test cannot see what it makes"),
            Item::ForeignMod(_) | Item::ExternCrate(_) | Item::Verbatim(_) => panic!("{path}: an item the surface test cannot read"),
            _ => {}
        }
    }
}

fn surface() -> BTreeSet<String> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let parsed = syn::parse_file(&fs::read_to_string(src.join("lib.rs")).unwrap()).unwrap();
    let mut out = BTreeSet::new();
    walk(&parsed.items, "mind_body", &src, &mut out);
    out
}

#[test]
fn the_librarys_public_surface_is_the_committed_one() {
    let actual = surface();
    let committed: BTreeSet<String> = SNAPSHOT.lines().filter(|line| !line.is_empty()).map(str::to_string).collect();
    let added: Vec<_> = actual.difference(&committed).collect();
    let removed: Vec<_> = committed.difference(&actual).collect();
    assert!(
        added.is_empty() && removed.is_empty(),
        "the public surface changed.\nadded:\n{}\nremoved:\n{}\nthe full current surface, for tests/public_surface.txt:\n{}",
        added.iter().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n"),
        removed.iter().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n"),
        actual.iter().cloned().collect::<Vec<_>>().join("\n"),
    );
}

#[test]
fn the_reader_sees_what_it_claims_to() {
    // The snapshot must hold the brake's own names, or the reader is blind to them.
    let actual = surface();
    for needle in ["mind_body::control: fn read_effective", "mind_body::control: struct Grant", "path-literal \"/etc/gamecult/minds\""] {
        assert!(actual.iter().any(|line| line.contains(needle)), "{needle} is not in the surface");
    }
    // And nothing under cfg(test) leaked in.
    assert!(!actual.iter().any(|line| line.contains("for_test") || line.contains("with_root") || line.contains("TEST_ROOT")), "a cfg(test) item is in the surface");
}
