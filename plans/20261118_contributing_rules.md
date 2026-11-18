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
- Kirk's local `AGENTS.md`: point to `CONTRIBUTING.md`, add the rules that
  are only for AI agents working in this repo.

## Acceptance criteria

- [ ] `bash scripts/tests/static-check.sh` passes (no code changed, but CI
      runs it)
- [ ] All links in `CONTRIBUTING.md` resolve
- [ ] Kirk has read `CONTRIBUTING.md` and agrees it says what he wants

## Decisions and failed attempts

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
