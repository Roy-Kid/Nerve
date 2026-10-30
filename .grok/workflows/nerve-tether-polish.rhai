let meta = #{
    name: "nerve-tether-polish",
    description: "Find one Nerve/Tether behavior gap, fix it, test it, and repeat until a round is dry",
    when_to_use: "Polish the macOS panel, the Tether web panel, and the hub reaper that feeds both. /nerve-tether-polish",
    phases: [
        #{ title: "Scan", detail: "read the panel and the reaper" },
        #{ title: "Verify", detail: "keep only gaps the code shows" },
        #{ title: "Fix", detail: "one gap, with a test" },
        #{ title: "Check", detail: "the diff matches the gap and the test ran" },
    ],
};

let gap_schema = #{
    "type": "object",
    "required": ["gaps"],
    "properties": #{
        "gaps": #{
            "type": "array",
            "maxItems": 5,
            "items": #{
                "type": "object",
                "required": ["id", "path", "evidence", "failure", "test", "severity"],
                "properties": #{
                    "id": #{ "type": "string" },
                    "path": #{ "type": "string" },
                    "evidence": #{ "type": "string" },
                    "failure": #{ "type": "string" },
                    "test": #{ "type": "string" },
                    "severity": #{ "type": "string" },
                },
            },
        },
    },
};
let verdict_schema = #{
    "type": "object",
    "required": ["real", "reason", "evidence"],
    "properties": #{
        "real": #{ "type": "boolean" },
        "reason": #{ "type": "string" },
        "evidence": #{ "type": "string" },
    },
};
let fix_schema = #{
    "type": "object",
    "required": ["changed", "tests", "summary"],
    "properties": #{
        "changed": #{ "type": "boolean" },
        "tests": #{ "type": "string" },
        "summary": #{ "type": "string" },
        "leftover": #{ "type": "string" },
    },
};
let check_schema = #{
    "type": "object",
    "required": ["pass", "reason"],
    "properties": #{
        "pass": #{ "type": "boolean" },
        "reason": #{ "type": "string" },
    },
};

fn has_prefix(path, prefix) {
    if type_of(path) != "string" { return false; }
    if path.len() < prefix.len() { return false; }
    path.sub_string(0, prefix.len()) == prefix
}

fn in_scope(path) {
    has_prefix(path, "surfaces/tether/")
        || has_prefix(path, "Nerve/Nerve/UI/Panel/")
        || has_prefix(path, "Nerve/Nerve/UI/MenuBar/")
        || has_prefix(path, "crates/nerve-hub/src/state/")
        || has_prefix(path, "crates/nerve-hub/src/hook/")
        || has_prefix(path, "crates/nerve-platform/src/pid/")
        || has_prefix(path, "plugins/nerve/hooks/")
        || has_prefix(path, "testing/tether/")
        || has_prefix(path, "crates/nerve-hub/tests/")
}

fn field(item, name) {
    if item == () || type_of(item) != "map" { return ""; }
    let value = item[name];
    if value == () || type_of(value) != "string" { return ""; }
    value
}

fn take_gaps(raw) {
    let kept = [];
    if raw == () || type_of(raw) != "array" { return kept; }
    let i = 0;
    for item in raw {
        if kept.len() >= 5 {
            log("dropped gaps past the cap of 5");
            break;
        }
        let path = field(item, "path");
        let evidence = field(item, "evidence");
        let failure = field(item, "failure");
        let rank = field(item, "severity");
        if path == "" || evidence == "" || failure == "" { i += 1; continue; }
        if rank != "high" && rank != "medium" {
            log("dropped " + path + " at severity " + rank);
            i += 1;
            continue;
        }
        if !in_scope(path) {
            log("dropped out of scope " + path);
            i += 1;
            continue;
        }
        kept.push(item);
        i += 1;
    }
    kept
}

let cap = 2;
if args != () && type_of(args) == "map" && args.round_cap != () {
    let n = args.round_cap;
    if (type_of(n) == "i64" || type_of(n) == "i32") && n >= 1 && n <= 3 { cap = n; }
}

let rounds = [];
let previous = "";
let round = 0;

while round < cap {
    round += 1;
    phase("Scan");
    let scan_prompt = "You are auditing /Users/roykid/work/Nerve. Use read_file and grep. Do not edit files. ";
    scan_prompt += "Read these before you conclude: crates/nerve-hub/src/state/reaper.rs, crates/nerve-platform/src/pid/mod.rs, ";
    scan_prompt += "crates/nerve-hub/src/hook/build.rs, plugins/nerve/hooks/nerve.js (agentPid), ";
    scan_prompt += "surfaces/tether/web/panel.js, surfaces/tether/web/model.js, Nerve/Nerve/UI/Panel/StatusPanelView.swift. ";
    scan_prompt += "Report at most 5 behavior gaps that are still true in the current files. ";
    scan_prompt += "A gap is in scope only when a user of the macOS status panel or the Tether panel can observe it, or when POST /v1/refresh fails to drop a local job whose producer process is gone. ";
    scan_prompt += "Required check: refresh reaps with kill(pid, 0) only. A recycled pid and a row with no extensions.pid both survive. If that is still the code, report it as severity high. ";
    scan_prompt += "Do not report Reduce Motion stopping the Tether lamp. That is intentional. ";
    scan_prompt += "Do not report Windows, VS Code, or tmux wording differences. Do not propose new features. ";
    scan_prompt += "Hub stays the only authority. Surfaces do not reap and do not infer status from free text. Silence is not exit. ";
    scan_prompt += "For each gap give id, path (repo-relative), evidence (a symbol you read), failure (one sentence), test (one command), severity high or medium. ";
    scan_prompt += "An empty gaps list is valid only after you have read the files above.";
    let scan = agent(scan_prompt, #{
        label: "scan:" + round.to_string(),
        phase: "Scan",
        capability_mode: "read-only",
        output_schema: gap_schema,
    });

    let found = [];
    if scan != () && scan.success && scan.output != () && type_of(scan.output) == "map" {
        found = take_gaps(scan.output.gaps);
    }
    if found.len() == 0 {
        log("round " + round.to_string() + " is dry");
        complete(#{
            summary: "Dry after " + round.to_string() + " round(s).",
            rounds: rounds,
        });
    }

    let ids = [];
    for gap in found { ids.push(field(gap, "id")); }
    let stamp = fingerprint(json_encode(ids));
    if stamp == previous {
        log("round " + round.to_string() + " repeated the same gaps");
        complete(#{
            summary: "Stopped: the same gaps came back.",
            rounds: rounds,
        });
    }
    previous = stamp;

    phase("Verify");
    let checks = [];
    for gap in found {
        let ask = "Adversarially verify this claimed gap by reading the file. Set real=true only if the evidence symbol is in the file and the failure is still true. ";
        ask += "Otherwise set real=false. Quote the lines you read in evidence. Claim: ";
        ask += json_encode(gap);
        checks.push(#{
            prompt: ask,
            label: "verify:" + field(gap, "id"),
            phase: "Verify",
            capability_mode: "read-only",
            output_schema: verdict_schema,
        });
    }
    let verdicts = parallel(checks);

    let confirmed = [];
    let index = 0;
    for verdict in verdicts {
        if verdict != () && verdict.success && verdict.output != ()
            && type_of(verdict.output) == "map"
            && verdict.output.real == true
            && field(verdict.output, "evidence") != "" {
            confirmed.push(found[index]);
        }
        index += 1;
    }
    log(confirmed.len().to_string() + "/" + found.len().to_string() + " gaps confirmed");
    if confirmed.len() == 0 {
        complete(#{
            summary: "Scan found gaps; none survived verification.",
            rounds: rounds,
        });
    }

    let chosen = confirmed[0];
    for gap in confirmed {
        if field(gap, "severity") == "high" {
            chosen = gap;
            break;
        }
    }

    phase("Fix");
    let fix_prompt = "Fix exactly one confirmed gap in /Users/roykid/work/Nerve. Edit the smallest set of files. ";
    fix_prompt += "Add or extend a test that fails on the old behavior and passes after your change. Run that test and paste the outcome into tests. ";
    fix_prompt += "Do not commit. Do not run ./scripts/nerve.sh. Do not edit Windows, VS Code, tmux, or the website. ";
    fix_prompt += "If the gap is the reaper: record the process start time next to extensions.pid and reap when the pid is gone OR the start time no longer matches. ";
    fix_prompt += "Keep today's rules: pid <= 1 is never probed, jobs younger than 3 seconds are kept, EPERM stays alive, a missing start time is not proof of death, the hub does not signal the agent. ";
    fix_prompt += "Silence is not exit. Set changed=false if you cannot fix it without breaking those rules. Gap: ";
    fix_prompt += json_encode(chosen);
    let fix = agent(fix_prompt, #{
        label: "fix:" + field(chosen, "id"),
        phase: "Fix",
        capability_mode: "execute",
        output_schema: fix_schema,
    });

    phase("Check");
    let check_prompt = "Check the uncommitted diff in /Users/roykid/work/Nerve with git diff. Set pass=true only if the diff addresses the gap, the named test exists, and the fixer reported a passing run. ";
    check_prompt += "Set pass=false if the diff touches unrelated surfaces, drops the EPERM-is-alive rule, or reaps on silence. Gap: ";
    check_prompt += json_encode(chosen);
    check_prompt += " Fixer result: ";
    if fix != () && fix.output != () { check_prompt += json_encode(fix.output); } else { check_prompt += "missing"; }
    let check = agent(check_prompt, #{
        label: "check:" + round.to_string(),
        phase: "Check",
        capability_mode: "read-only",
        output_schema: check_schema,
    });

    let passed = false;
    let why = "check did not return";
    if check != () && check.success && check.output != () && type_of(check.output) == "map" {
        passed = check.output.pass == true;
        why = field(check.output, "reason");
    }
    let note = field(chosen, "id");
    if fix != () && fix.output != () && type_of(fix.output) == "map" {
        note += " " + field(fix.output, "summary");
    }
    rounds.push(#{ round: round, id: field(chosen, "id"), pass: passed, reason: why, note: note });
    if !passed {
        log("round " + round.to_string() + " failed the check");
        complete(#{
            summary: "Stopped after a failed check on " + field(chosen, "id") + ".",
            rounds: rounds,
        });
    }
    log("round " + round.to_string() + " landed " + field(chosen, "id"));
}

complete(#{
    summary: "Finished " + cap.to_string() + " round(s).",
    rounds: rounds,
});
