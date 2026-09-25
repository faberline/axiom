# Active Codex prompt-only fleet

The renderer source is `scripts/agents/render_fleet.py`.

The Codex 98-role projection is active. It provides PM/TL/QA/Dev roles for 23 apps,
custom `aw-dev`, and six singleton roles. Generate it only from
the templates with `scripts/agents/render_fleet.py`; never hand-edit a
generated role file.

This fleet uses prompt-only scope control. The controller gives every task its
exact paths, permits one writer in one worktree, and retains Git, tracker, and
acceptance authority. This is not permission isolation.

The CLI canary allowed all six forbidden writes. A native read-only
`lumen-qa` canary also wrote its first `/private/tmp` sentinel. The desktop
admission test ignored the installed hook and launched `lumen-dev`; it did no
tool or write action. The global configuration was restored. Only the
`lumen-qa` Luna/max declaration was measured. These results do not prove
permission isolation or hook enforcement for Codex or Claude.

The prior rollback retained the candidate archive at
`/private/tmp/axiom-rollback-reviewed/current-candidate.tar.gz`. Its
`moved-new` directory and `rollback-journal.json` are in the same directory.
The recovered `integration-qa` singleton is now part of the active fleet.

`python3 -B scripts/agents/render_fleet.py --check-codex` must exit zero. This
does not generate or activate Claude roles. Permission profiles, tool
isolation, and hook enforcement remain separate work. Do not claim that the
active prompt-only fleet has any of those protections.
