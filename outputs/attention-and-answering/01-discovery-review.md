# Discovery review: agents-ui · Attention and answering

| Field | Value |
|---|---|
| Reviewed | .pm/drafts/01-discovery.md, before the priority check-in |
| Reviewer | nlyu-pm:discovery-reviewer (read-only) |
| Reviewed at | 2026-11-02 |
| Result | 0 blocking · 8 major · 5 minor · quotes 10/10 verified · floor met |

## Summary
All ten quotes match their saved fetches word for word and all five constraint lines are at their cited paths. Nothing is blocking; the major findings were about how strongly the brief states its evidence (source-type counts, listing pages labelled Evidence, unreliable reaction counts). The coverage floor is met: approved allowed.

## Findings and disposition
| ID | Severity | Area | Finding | Where | Disposition |
|---|---|---|---|---|---|
| RV-001 | major | label | SIG-004, SIG-015 (search listings) and SIG-016 (forum blurbs) were labelled Evidence | 4c | fixed in brief (relabelled Inference, as SIG-014) |
| RV-002 | major | label | Listing-page numbers are not thumbs-up counts; "144 thumbs-up", "9 to 49 reactions", "top item 18", and the "5,000+" base are unreliable | 4c SIG-004, -014, -015; OPP-003 | fixed in brief (numbers dropped or re-described; base corrected to 989 of 5,794 matching "notification") |
| RV-003 | major | label | SIG-016 overstated: "several build their own UI" rests on one post; post 758 asks about approval by email | 4c SIG-016 | fixed in brief (claim restated with posts 19653 and 758) |
| RV-004 | major | label | Rung-2 items and SIG-010 cite permalinks, not the fetched search URLs | 4c | fixed in brief (a "Fetched as" line with the saved URL added to each) |
| RV-005 | major | opportunity | OPP-001 counted channels as source types | OPP-001 | fixed in brief (6 signals, 5 Evidence, 2 source types) |
| RV-006 | major | opportunity | OPP-003 overstated: SIG-011 cited as support and counter, SIG-014 is Inference, SIG-003 and SIG-013 the same issue | OPP-003 | fixed in brief (4 signals, 3 Evidence, 2 source types; SIG-011 out of support) |
| RV-007 | major | triangulation | The tmux fix was released by March 2026 with mixed reports; August is the issue's closing | Section 2, Theme A | fixed in brief |
| RV-008 | major | opportunity | OPP-001 reads GitHub issues; not tested against the third-party key or OAuth exclusion | OPP-001 | PM decided (D-003): reading through the PM's own signed-in gh is allowed |
| RV-009 | minor | opportunity | Exclusions checks did not name each case exclusion | OPP-001 to 003 | fixed in brief (all seven named) |
| RV-010 | minor | triangulation | "Who says so" counts did not match cited items | Section 2 | fixed in brief |
| RV-011 | minor | label | SIG-010 base was the first page of 100 items, not the repository | 4c SIG-010 | fixed in brief |
| RV-012 | minor | triangulation | Rung-1 items sat in the "Numbers, competitors, official" column | 4a | fixed in brief (moved to the rung-1 column) |
| RV-013 | minor | counter-evidence | SIG-002 and SIG-011 used as both support and counter-evidence | 4a | accepted with note (a fetch stands behind each; the double use is now stated in the table) |

## Quote verification
| Quote | Saved file | Result | Note |
|---|---|---|---|
| QUO-001 | F-008 | verified | |
| QUO-002 | F-009 | verified | |
| QUO-003 | F-011 | verified | |
| QUO-004 | F-011 | verified | |
| QUO-005 | F-006 | verified | |
| QUO-006 | F-012 | verified | |
| QUO-007 | F-007 | verified | |
| QUO-008 | F-007 | verified | |
| QUO-009 | F-008 | verified | |
| QUO-010 | F-006 | verified | |

## Coverage floor
| Floor | Minimum | Found | Met |
|---|---|---|---|
| Rungs with at least one Evidence item | 2 | 2 | yes |
| Evidence items in total | 5 | 12 after relabelling | yes |
| CON-### whose quoted line was confirmed | 2 | 5 | yes |
| Quotes per opportunity verified or PM-held | 1 | OPP-001: 3, OPP-002: 3, OPP-003: 1 | yes |

Verdict: approved allowed

## Notes for the PM
- The constraint lines were checked in the current working tree; the code has moved past the pinned commit 0019fe8, and all five lines are still at their cited places.
- OPP-003 is the thinnest: three Evidence items from two kinds of source.
