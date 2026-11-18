# Contributing rules

## Goal

Write down how changes get into VorT2x, for people and for AI tools: work on
branches, a human test note before merging, pull requests approved by Kirk,
plans for non-trivial work, AI disclosure and focused changes, and shared
coding standards.

## Plan

- `CONTRIBUTING.md`: the public rules (GitHub links it from every pull
  request).
- `plans/TEMPLATE.md`: a starting point for plan documents.
- `.github/pull_request_template.md`: plan link, AI section, testing
  checklist.
- README: link `CONTRIBUTING.md` from the Contributing section.
- `.github/workflows/installer.yml`: run only on `main` and `v*` tags.
- Kirk's local `AGENTS.md`: point to `CONTRIBUTING.md`, add the rules that
  are only for AI agents working in this repo.

## Acceptance criteria

- [x] `bash scripts/tests/static-check.sh` passes
- [x] All links in `CONTRIBUTING.md` resolve
- [x] `installer.yml` no longer runs on pull requests, still runs on pushes
      to `main` and on `v*` tags

## Decisions and failed attempts

- 2026-11-18: Kirk: CI runs only when work reaches `main` (and on release
  tags, which build the release), not on branches or pull requests. Removed
  the `pull_request` trigger from `installer.yml`.
- 2026-11-18: The PR was first opened with the acceptance criteria unticked.
  Kirk: they must all be ticked before a PR is raised. One criterion was
  "Kirk has read CONTRIBUTING.md and agrees", which only Kirk can tick; that
  is what his PR approval is for, so it was removed rather than ticked. Rule
  added: acceptance criteria are checks that can be run before the PR;
  hardware testing goes in the Tested note.
- 2026-11-18: The rules go in a public `CONTRIBUTING.md`, not only in
  `AGENTS.md`, because `AGENTS.md` is local-only (it holds Kirk's private
  notes) and contributors would never see it.
- 2026-11-18: AI is disclosed in the pull request description (and, for
  kernel patches, the commit message as the kernel requires), not with
  `Co-Authored-By` lines, which Kirk doesn't use.
- 2026-11-18: The human "ready to merge" note goes on the pull request (and
  optionally the plan's Tested section), so it sits with the branch.

## Tested

Documentation only; nothing to test on hardware.
