# Evidence review: A feature from spec to production (Scout run 2)

Reviewer: the nlyu-pm discovery-reviewer agent (read-only), 2026-11-02, against `.pm/drafts/01-discovery.md`, fetches F-016 to F-032, and laird/agents-ui at e4422c5 (lines re-checked at the current head).

**Verdict:** all 8 quotes are verbatim in their saved fetches, and all 5 code constraints are at the cited lines. The problems were about weight, not fabrication. All three opportunities (OPP-004, OPP-005, OPP-006) are **provisional**: the coverage floor for market or competitor observations is not met (1 of 2).

| ID | Severity | Finding | Disposition |
|---|---|---|---|
| RV-001 | blocking | Market or competitor observations: 1 of the 2 required (SIG-017) | Open. The PM's call: waive it (community-only sources were chosen for this run) or research one more |
| RV-002 | major | The "69 votes" are on GitHub's own announcement, not on user requests | Fixed: the requests (21 and 20 votes) are cited as demand |
| RV-003 | major | "Roll-up still asked for" was wrong: GitHub's API already has `sub_issues_summary` | Fixed |
| RV-004 | major | laird/agents #146 and #148 are the product owner's own proposals | Fixed: counted separately from customer voice |
| RV-005 | major | SIG-022's source was given as an unfetched Reddit URL, with "date not shown" | Fixed: Lemmy post URL, 2026-09-12, score -1, 0 comments |
| RV-006 | major | SIG-025 (ship small, often) is context, not demand | Fixed: removed from OPP-005's support |
| RV-007 | major | Theme B was called fully open, but GitHub Deployments records the sha per environment | Fixed: "partly addressed; open per feature" |
| RV-008 | major | Theme A's counter-evidence was weak and not from a dedicated search | Fixed: labelled weak, and F-024 42761674 added as a counter |
| RV-009 | major | Theme C was called "recurring" without one direct signal | Fixed by RV-010 |
| RV-010 | major | On-topic QA, UAT and pre-production sign-off comments in F-017 and F-026 were set aside | Fixed: logged as SIG-032 to SIG-034, with QUO-019 to QUO-021 (each checked verbatim) |
| RV-011 | minor | Community count was 12; actually 11 | Fixed (14 with the new signals) |
| RV-012 | minor | Supply signals (SIG-030, SIG-031) were counted as demand | Fixed: demand and supply counted separately |
| RV-013 | minor | OPP-005 cited CON-007 without listing it | Fixed |
| RV-014 | minor | Case and session files still pinned 0019fe8 | Fixed: re-pinned to e4422c5 |
| RV-015 | minor | CON-007's quote didn't show the claim | Fixed: describes the struct's whole field span |
| RV-016 | minor | Reading sub-issues and Deployments extends D-003; GitHub Environments may depend on the plan for private repositories | Noted in the exclusions checks, to confirm before building |
| RV-017 | minor | SIG-030 left out the non-transient and auto_inactive caveats | Fixed |
| RV-018 | minor | OPP-006 left out QUO-018, and QUO-015 linked nowhere | Fixed |
