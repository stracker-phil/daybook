//! Reading `plaunch`'s answers. Pure.

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

/// A project Project Launcher lists.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "LauncherProject")]
pub struct ListedProject {
    pub id: String,
    pub name: String,
    pub archived: bool,
}

fn listed(entry: &Value) -> Option<ListedProject> {
    let id = entry.get("id")?.as_str().filter(|id| !id.is_empty())?;
    let name = entry
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .unwrap_or(id);
    Some(ListedProject {
        id: id.to_string(),
        name: name.to_string(),
        archived: entry.get("archived") == Some(&Value::Bool(true)),
    })
}

/// `plaunch list --json`: entries without an id are dropped, a missing name is the id; anything but a
/// list is no projects. `Err` when it isn't JSON.
pub fn parse_projects(stdout: &str) -> Result<Vec<ListedProject>, String> {
    let value: Value = serde_json::from_str(stdout)
        .map_err(|e| format!("Project Launcher's answer isn't JSON: {e}"))?;
    Ok(value
        .as_array()
        .map(|entries| entries.iter().filter_map(listed).collect())
        .unwrap_or_default())
}

/// What a failed run says: its stderr without the `plaunch:` prefix, else `fallback`.
pub fn failure_message(stderr: &str, fallback: &str) -> String {
    let text = stderr.trim();
    let text = text.strip_prefix("plaunch:").map_or(text, str::trim_start);
    if text.is_empty() {
        fallback.to_string()
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: &str, name: &str, archived: bool) -> ListedProject {
        ListedProject {
            id: id.into(),
            name: name.into(),
            archived,
        }
    }

    #[test]
    fn entries_without_an_id_are_dropped_and_a_missing_name_is_the_id() {
        let stdout = r#"[{ "id": "proj-0001", "name": "Warehouse", "archived": false, "path": "/Users/x/Code/warehouse" },{ "id": "proj-0002", "name": "", "archived": true },{ "id": "", "name": "No id" },{ "name": "Missing id" },"junk",{ "id": "proj-0003", "name": 7, "archived": "yes" }]"#;
        assert_eq!(
            parse_projects(stdout).unwrap(),
            vec![
                project("proj-0001", "Warehouse", false),
                project("proj-0002", "proj-0002", true),
                project("proj-0003", "proj-0003", false),
            ]
        );
    }

    #[test]
    fn an_answer_that_is_not_a_list_is_no_projects() {
        for stdout in ["{}", r#""x""#, "[]"] {
            assert_eq!(parse_projects(stdout).unwrap(), vec![], "{stdout}");
        }
    }

    #[test]
    fn an_answer_that_is_not_json_is_an_error() {
        assert!(parse_projects("not json").is_err());
    }

    #[test]
    fn a_listed_project_serializes_with_its_three_fields() {
        assert_eq!(
            serde_json::to_value(project("p", "P", true)).unwrap(),
            serde_json::json!({ "id": "p", "name": "P", "archived": true })
        );
    }

    #[test]
    fn a_failure_message_drops_the_plaunch_prefix_and_whitespace() {
        for (stderr, expected) in [
            ("plaunch: boom\n", "boom"),
            ("  plaunch:    boom  ", "boom"),
            ("boom", "boom"),
            ("  no prefix \n", "no prefix"),
            ("", "fallback"),
            ("  \n", "fallback"),
            ("plaunch:", "fallback"),
        ] {
            assert_eq!(failure_message(stderr, "fallback"), expected, "{stderr:?}");
        }
    }
}
