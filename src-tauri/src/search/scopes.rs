//! Search scopes (Settings → Search) as folder prefixes, and the folders search never covers.

use std::collections::HashMap;

use crate::store::settings::{Context, SearchScope, Settings};

/// The scope that covers the whole vault; it always exists.
pub const ALL: &str = "all";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resolved {
    /// Each scope's folder prefixes (`""` is the whole vault), in settings order, then `all` when the
    /// settings don't define it.
    pub scopes: Vec<(String, Vec<String>)>,
    /// Unknown scopes and cycles, once each.
    pub problems: Vec<String>,
}

impl Resolved {
    pub fn names(&self) -> Vec<String> {
        self.scopes.iter().map(|(name, _)| name.clone()).collect()
    }

    pub fn get(&self, name: &str) -> Option<&[String]> {
        self.scopes
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, prefixes)| prefixes.as_slice())
    }

    /// The union of the known `names`' prefixes; the whole vault (`[""]`) when none is known.
    pub fn prefixes(&self, names: &[String]) -> Vec<String> {
        let mut prefixes: Vec<String> = Vec::new();
        let mut known = false;
        for found in names.iter().filter_map(|name| self.get(name)) {
            known = true;
            for prefix in found {
                if !prefixes.contains(prefix) {
                    prefixes.push(prefix.clone());
                }
            }
        }
        if known { prefixes } else { vec![String::new()] }
    }
}

fn is_folder(entry: &str) -> bool {
    entry == "/" || entry.ends_with('/')
}

/// `2-Work/` → `2-Work/`, `/` → `""`.
fn folder_prefix(entry: &str) -> String {
    let clean = entry.trim().trim_start_matches('/').trim_end_matches('/');
    if clean.is_empty() {
        String::new()
    } else {
        format!("{clean}/")
    }
}

struct Resolver<'a> {
    definitions: HashMap<&'a str, &'a [String]>,
    resolved: HashMap<String, Vec<String>>,
    problems: Vec<String>,
}

impl Resolver<'_> {
    fn problem(&mut self, problem: String) {
        if !self.problems.contains(&problem) {
            self.problems.push(problem);
        }
    }

    fn resolve(&mut self, name: &str, trail: &mut Vec<String>) -> Vec<String> {
        if let Some(found) = self.resolved.get(name) {
            return found.clone();
        }
        if trail.iter().any(|seen| seen == name) {
            self.problem(format!("Scope “{name}” includes itself."));
            return Vec::new();
        }
        let Some(entries) = self.definitions.get(name).copied() else {
            let from = trail.last().cloned().unwrap_or_default();
            self.problem(format!(
                "Scope “{from}” includes the unknown scope “{name}”."
            ));
            return Vec::new();
        };
        trail.push(name.to_string());
        let mut folders: Vec<String> = Vec::new();
        for entry in entries {
            let found = if is_folder(entry) {
                vec![folder_prefix(entry)]
            } else {
                self.resolve(entry, trail)
            };
            for folder in found {
                if !folders.contains(&folder) {
                    folders.push(folder);
                }
            }
        }
        trail.pop();
        self.resolved.insert(name.to_string(), folders.clone());
        folders
    }
}

/// Each scope's folders; an entry ending in `/` is a folder, anything else names another scope.
pub fn resolve(scopes: &[SearchScope]) -> Resolved {
    let mut resolver = Resolver {
        definitions: scopes
            .iter()
            .map(|scope| (scope.name.as_str(), scope.entries.as_slice()))
            .collect(),
        resolved: HashMap::new(),
        problems: Vec::new(),
    };
    let mut names: Vec<&str> = Vec::new();
    for scope in scopes {
        if !names.contains(&scope.name.as_str()) {
            names.push(&scope.name);
        }
    }
    let mut resolved: Vec<(String, Vec<String>)> = names
        .iter()
        .map(|name| (name.to_string(), resolver.resolve(name, &mut Vec::new())))
        .collect();
    if !names.contains(&ALL) {
        resolved.push((ALL.into(), vec![String::new()]));
    }
    Resolved {
        scopes: resolved,
        problems: resolver.problems,
    }
}

/// Settings → Search → Never search as path prefixes: `/2-Work/` → `2-Work/`, `_/*` → `_/`, `/` → `""`;
/// empty entries are dropped.
pub fn exclude_prefixes(exclude: &[String]) -> Vec<String> {
    exclude
        .iter()
        .map(|entry| {
            let entry = entry.trim();
            let entry = match entry.strip_suffix("/*") {
                Some(folder) => format!("{folder}/"),
                None => entry.to_string(),
            };
            entry.trim_start_matches('/').to_string()
        })
        .filter(|prefix| !prefix.is_empty())
        .collect()
}

/// The context's search scope when it names a scope other than `all`.
pub fn default_scope(settings: &Settings, context: Option<&Context>) -> Option<String> {
    let scope = &context?.search_scope;
    let known = resolve(&settings.search.scopes).get(scope).is_some();
    (known && scope != ALL).then(|| scope.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(name: &str, entries: &[&str]) -> SearchScope {
        SearchScope {
            name: name.into(),
            entries: entries.iter().map(|entry| entry.to_string()).collect(),
        }
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    fn journal_and_work() -> Resolved {
        resolve(&[
            scope("journal", &["1-Journal/"]),
            scope("work", &["2-Work/", "journal"]),
        ])
    }

    #[test]
    fn a_scope_includes_the_folders_of_the_scopes_it_names() {
        let resolved = resolve(&[
            scope("journal", &["1-Journal/"]),
            scope("work", &["2-Work/", "journal", "nope"]),
        ]);
        assert_eq!(
            resolved.get("work"),
            Some(&strings(&["2-Work/", "1-Journal/"])[..])
        );
        assert_eq!(resolved.get("all"), Some(&strings(&[""])[..]));
        assert_eq!(
            resolved.problems,
            vec!["Scope “work” includes the unknown scope “nope”."]
        );
    }

    #[test]
    fn a_scope_that_includes_itself_is_a_problem_and_adds_nothing() {
        let resolved = resolve(&[scope("x", &["x", "a/"])]);
        assert_eq!(resolved.problems, vec!["Scope “x” includes itself."]);
        assert_eq!(resolved.get("x"), Some(&strings(&["a/"])[..]));
    }

    #[test]
    fn a_cycle_between_scopes_is_reported_once() {
        let resolved = resolve(&[scope("a", &["b"]), scope("b", &["a"])]);
        assert_eq!(resolved.problems, vec!["Scope “a” includes itself."]);
    }

    #[test]
    fn the_same_problem_is_reported_once() {
        let resolved = resolve(&[scope("w", &["nope", "nope"])]);
        assert_eq!(
            resolved.problems,
            vec!["Scope “w” includes the unknown scope “nope”."]
        );
    }

    #[test]
    fn folder_entries_become_prefixes_with_one_trailing_slash() {
        for (entry, expected) in [
            ("/", ""),
            ("2-Work/", "2-Work/"),
            ("/2-Work//", "2-Work/"),
            ("a/b/", "a/b/"),
        ] {
            let resolved = resolve(&[scope("s", &[entry])]);
            assert_eq!(
                resolved.get("s"),
                Some(&strings(&[expected])[..]),
                "{entry}"
            );
        }
    }

    #[test]
    fn folders_reached_twice_are_listed_once_in_first_seen_order() {
        let resolved = resolve(&[scope("j", &["a/"]), scope("w", &["b/", "j", "a/"])]);
        assert_eq!(resolved.get("w"), Some(&strings(&["b/", "a/"])[..]));
    }

    #[test]
    fn all_is_added_last_only_when_the_settings_do_not_define_it() {
        let added = resolve(&[scope("a", &["a/"])]);
        assert_eq!(added.names(), strings(&["a", "all"]));
        assert_eq!(added.get("all"), Some(&strings(&[""])[..]));

        let defined = resolve(&[scope("all", &["x/"]), scope("a", &["a/"])]);
        assert_eq!(defined.names(), strings(&["all", "a"]));
        assert_eq!(defined.get("all"), Some(&strings(&["x/"])[..]));
    }

    #[test]
    fn names_follow_the_settings_order_and_an_unknown_scope_has_no_folders() {
        let resolved = journal_and_work();
        assert_eq!(resolved.names(), strings(&["journal", "work", "all"]));
        assert_eq!(resolved.get("nope"), None);
    }

    #[test]
    fn prefixes_are_the_union_of_the_known_scopes() {
        let resolved = journal_and_work();
        let names = |items: &[&str]| strings(items);
        assert_eq!(
            resolved.prefixes(&names(&["journal", "work"])),
            strings(&["1-Journal/", "2-Work/"])
        );
        assert_eq!(
            resolved.prefixes(&names(&["nope", "journal"])),
            strings(&["1-Journal/"])
        );
    }

    #[test]
    fn prefixes_cover_the_whole_vault_when_no_scope_is_known() {
        let resolved = journal_and_work();
        assert_eq!(resolved.prefixes(&[]), strings(&[""]));
        assert_eq!(resolved.prefixes(&strings(&["nope"])), strings(&[""]));
    }

    #[test]
    fn excluded_folders_become_path_prefixes() {
        let exclude = strings(&[
            " /_/* ", "_/", "/", "", "  ", ".trash/", "docs/*", "/*", "_/*",
        ]);
        assert_eq!(
            exclude_prefixes(&exclude),
            strings(&["_/", "_/", ".trash/", "docs/", "_/"])
        );
    }

    fn settings_with_context_scope(scope: &str) -> (Settings, Context) {
        let settings = Settings::default();
        let mut context = settings.contexts[0].clone();
        context.search_scope = scope.into();
        (settings, context)
    }

    #[test]
    fn the_default_scope_is_the_contexts_scope_when_it_is_known_and_not_all() {
        for (name, expected) in [("work", Some("work")), ("all", None), ("nope", None)] {
            let (settings, context) = settings_with_context_scope(name);
            assert_eq!(
                default_scope(&settings, Some(&context)),
                expected.map(String::from),
                "{name}"
            );
        }
    }

    #[test]
    fn without_a_context_there_is_no_default_scope() {
        assert_eq!(default_scope(&Settings::default(), None), None);
    }
}
