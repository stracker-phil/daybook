#!/usr/bin/env python3
"""Fills an empty debug data folder and a vault folder with a made-up day, for screenshots.

Usage: prepare.py <data-dir> <vault-dir>   (called by scripts/dev/demo/screens.sh)
The settings start from the app's defaults (src/test/default-settings.json) with demo contexts, projects and
links; TimeTracker is the stand-in next to this file, and Slack and Jira stay off.
"""
import json
import sys
from datetime import date, timedelta
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
DATA, VAULT = Path(sys.argv[1]), Path(sys.argv[2])
TODAY = date.today()


def link(title, target, **extra):
    return {"title": title, "target": target, "contexts": ["work"], "projects": None, "days": None, "time": None,
            "event": None, "eventWindow": "until_end", "leadMinutes": 15, "pinned": False, **extra}


def project(name, emoji, keywords, linked):
    return {"name": name, "slackEmoji": emoji, "active": True, "contexts": ["work"], "keywords": keywords,
            "launcherProjects": [], "timetrackerProjects": [{"id": i, "name": n} for i, n in linked]}


def settings():
    s = json.loads((ROOT / "src/test/default-settings.json").read_text())
    s["launchAtLogin"] = False
    s["vault"] = {"path": str(VAULT), "name": "Demo", "obsidianCliPath": ""}
    work, private = s["contexts"]
    work.update(label="💼", calendars=["Work"], features=["status", "routines"])
    private.update(label="🏡", name="Personal")
    s["projects"] = [
        project("Internal", "", ["standup", "planning"], [("p-internal", "Internal")]),
        project("Acme", ":acme:", ["acme"], [("p-acme-web", "Web Shop"), ("p-acme-app", "Mobile App")]),
        project("Globex", ":globex:", ["globex", "report"], [("p-globex", "Globex")]),
    ]
    s["links"] = [
        link("Team board", "https://example.com/board", pinned=True),
        link("Acme design system", "https://example.com/acme/design", projects=["Acme"]),
        link("Standup notes", "2-Work/Standup.md", event="standup"),
        link("Quarterly report draft", "2-Work/Globex report.md", projects=["Globex"]),
    ]
    s["search"]["scopes"] = [{"name": "work", "entries": ["1-Journal/", "2-Work/"]}, {"name": "all", "entries": ["/"]}]
    s["timetracker"] = {"path": str(HERE / "timetracker"), "linked": True, "statusTimer": True}
    s["jira"] = {"baseUrl": "", "defaultPrefix": ""}
    s["slack"]["channel"] = ""
    s["api"]["enabled"] = False
    return s


def state():
    day = TODAY.isoformat()
    return {
        "settings.json": settings(),
        "context.json": {"id": "work", "setAt": f"{day}T07:45:00.000Z"},
        "status-memory.json": {"date": day, "status": "working", "since": "08:45", "workedToday": True},
        "activity.json": {"day": day, "activity": {"project": "Acme", "note": "ACME-139: Login fails offline"}},
        # Everything that opens by itself today already did, so nothing pops up while capturing.
        "routines.json": {"date": day, "fired": [
            "routine:morning@due", "routine:morning@start", "routine:evening@due", "routine:evening@off",
            "journal:Dawn@due", "journal:Dusk@due"]},
        "ui_state.json": {"sidebarHidden": False, "inspectorHidden": False, "hidePastEvents": False,
                          "collapsedTodoGroups": [], "inspectorTab": "todos", "activityView": "timeline"},
    }


TODAY_NOTE = """## Journal

> [!NOTE]+ Dawn
> **What am I grateful for?** - _Small things count too_
> 1. A slow coffee before the first call
> 2. The checkout redesign shipped
>
> **My current goal** - _Where do I want to be in a year?_
> Ship the mobile app without weekend work
>
> **The "One Thing"** - _What would I do today even without pay?_
> - [ ] Sketch the new onboarding flow

## Focus
- `[x]` :acme: Review the checkout pull request
- `[/]` :acme: Fix the offline login bug
- `[ ]` :globex: Draft the quarterly report

## Plan for Today
- [x] Plan the day
- [x] Check the team board
- [ ] Review open pull requests

## Notes

### Acme sync
- Release candidate on Thursday
- Ask about the analytics events

### Design review
- Onboarding: fewer steps, one screen per question
"""

EARLIER_NOTE = """## Plan for Today
- [x] Plan the day

## Notes

### Follow-ups
- [ ] Send the invoice to Globex
- [ ] Book the team offsite venue
"""


def vault():
    journal = VAULT / "1-Journal"
    journal.mkdir(parents=True, exist_ok=True)
    (journal / f"{TODAY.isoformat()}.md").write_text(TODAY_NOTE)
    (journal / f"{(TODAY - timedelta(days=2)).isoformat()}.md").write_text(EARLIER_NOTE)
    work = VAULT / "2-Work"
    work.mkdir(exist_ok=True)
    (work / "Standup.md").write_text("# Standup\n\n- Yesterday, today, blockers\n")
    (work / "Globex report.md").write_text("# Quarterly report\n\n## Highlights\n\n## Risks\n")
    (work / "Onboarding ideas.md").write_text("# Onboarding ideas\n\n- One question per screen\n- Skip for later\n")


def main():
    DATA.mkdir(parents=True, exist_ok=True)
    for name, content in state().items():
        (DATA / name).write_text(json.dumps(content, indent=2, ensure_ascii=False))
    vault()


main()
