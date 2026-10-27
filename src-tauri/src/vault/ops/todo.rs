//! Ticking an open todo of the inspector: `[x]` into the one line the index read it from, refused when that line
//! no longer holds the todo (edited, moved or ticked in Obsidian meanwhile). Pure.

use crate::todos::scan::{open_task_text, todo_key};

pub const TODO_CHANGED: &str =
    "This todo changed in its note since Daybook read it. Look at it in Obsidian.";

pub fn tick_todo(lines: &mut [String], line: usize, text: &str) -> Result<(), String> {
    let current = lines.get_mut(line).ok_or(TODO_CHANGED)?;
    let holds = open_task_text(current).is_some_and(|held| todo_key(held) == todo_key(text));
    if !holds {
        return Err(TODO_CHANGED.into());
    }
    *current = current.replacen("[ ]", "[x]", 1);
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::store::settings::Settings;
    use crate::vault::ops::{NoteLayout, NoteOp};

    fn tick(line: u32, text: &str) -> NoteOp {
        NoteOp::TodoDone {
            line,
            text: text.into(),
        }
    }

    fn apply(op: &NoteOp, text: &str) -> Result<Option<String>, String> {
        op.apply(text, &NoteLayout::from_settings(&Settings::default()))
    }

    #[test]
    fn ticking_checks_exactly_the_given_line_and_leaves_duplicates() {
        assert_eq!(
            apply(
                &tick(3, "Call Anna"),
                "# Day\n- [ ] Call Anna\n\n- [ ] Call Anna\n"
            ),
            Ok(Some("# Day\n- [ ] Call Anna\n\n- [x] Call Anna\n".into()))
        );
    }

    #[test]
    fn a_todo_in_a_callout_is_ticked_inside_the_callout() {
        assert_eq!(
            apply(&tick(1, "x"), "> [!NOTE]\n> - [ ] x\n"),
            Ok(Some("> [!NOTE]\n> - [x] x\n".into()))
        );
    }

    #[test]
    fn case_and_spacing_differences_still_match_and_stay_as_written() {
        assert_eq!(
            apply(&tick(0, "call anna"), "- [ ]  Call   ANNA\n"),
            Ok(Some("- [x]  Call   ANNA\n".into()))
        );
    }

    #[test]
    fn a_line_that_no_longer_holds_the_todo_is_refused() {
        let text = "- [ ] Call Anna edited\n- [x] Call Anna\n- [ ] Other\nplain\n";
        for (line, label) in [
            (0, "edited"),
            (1, "already ticked"),
            (2, "another todo"),
            (3, "not a task"),
            (4, "out of range"),
            (u32::MAX, "far out of range"),
        ] {
            assert_eq!(
                apply(&tick(line, "Call Anna"), text),
                Err(TODO_CHANGED.to_string()),
                "{label}"
            );
        }
    }

    #[test]
    fn ticking_keeps_crlf_line_endings() {
        assert_eq!(
            apply(&tick(1, "a"), "# Day\r\n- [ ] a\r\n- [ ] b\r\n"),
            Ok(Some("# Day\r\n- [x] a\r\n- [ ] b\r\n".into()))
        );
    }

    #[test]
    fn the_op_is_described_as_the_line_it_writes() {
        assert_eq!(tick(3, "Call Anna").describe(), "Todo: - [x] Call Anna");
    }

    #[test]
    fn the_op_round_trips_as_json() {
        let shape = json!({"kind": "todoDone", "line": 3, "text": "x"});
        let op: NoteOp = serde_json::from_value(shape.clone()).unwrap();
        assert_eq!(op, tick(3, "x"));
        assert_eq!(serde_json::to_value(&op).unwrap(), shape);
    }
}
