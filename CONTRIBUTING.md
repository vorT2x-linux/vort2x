# Contributing to VorT2x

Thanks for helping. VorT2x is an installer: run once on macOS to make a
Fedora USB stick, then once in Fedora to set up T2 Mac support. Changes that
keep it simple, reliable and focused on that job are the most welcome.

Please be kind. Everyone here is a volunteer. See [VALUES.md](VALUES.md).

## How a change gets in

1. **Fork** the repository and work on a **branch** in your fork. Nothing is
   committed straight to `main`, by anyone.
2. For anything non-trivial, start with a **plan** (see [Plans](#plans)).
3. **Tick every acceptance criterion** in the plan, then open a **pull
   request** against `main`. Fill in the template, including the AI
   section. A pull request with unticked criteria isn't ready to open.
4. **Test it** on a real T2 Mac. When it's tested and working, a person (not
   an AI) adds a short note to the pull request saying it's ready to merge:
   who tested it, on which Mac model and Fedora version, and what they
   checked.
5. **Kirk approves** every pull request before it's merged.

After your first contribution has been merged, you can ask to become a
direct contributor. Direct contributors push branches to this repository
instead of a fork, but the rest stays the same: branch, pull request, test
note, Kirk's approval.

Version numbers and releases are decided by the maintainer as they come up.

## Plans

Every non-trivial change starts with a short, simple plan:

- saved in `plans/` as `YYYYMMDD_{slug}.md`, for example
  `plans/20261118_touchbar_brightness.md`;
- committed as part of the pull request;
- with **acceptance criteria**: a concrete list of tests that must pass,
  all ticked before the pull request is opened (so only checks you can run
  yourself; testing on a Mac goes in the Tested note);
- with a **history of decisions and failed attempts**, for example "reading
  the sensor through hwmon worked but lagged by two seconds".

A typo fix or a one-line change doesn't need a plan. If you're unsure, write
one; a few lines is fine. [plans/TEMPLATE.md](plans/TEMPLATE.md) is a
starting point.

## Using AI

AI tools are welcome. Two rules:

- **Disclose it.** Say in the pull request which tool you used and what it
  did (wrote the code, wrote the tests, reviewed, explained). For kernel
  patches, also follow the kernel's own rules
  (`Documentation/process/coding-assistants.rst`): say so in the commit
  message, and never add a `Signed-off-by` line for an AI.
- **Keep it focused.** One problem per pull request, with the smallest change
  that solves it. No large AI-driven rewrites or sweeping refactors; if you
  think something needs restructuring, open an issue first.

You're responsible for every line you submit, however it was written. Read
it, understand it and test it.

## Coding standards

- Aim for simple, readable implementations.
- Comments explain **why** the code is written a particular way, not what it
  does.
- Performance matters: look for inefficiencies.
- Memory use matters: look for wasteful allocations and copies.
- Before writing code, check whether an existing dependency already solves
  the problem, and use it rather than hand-rolling an equivalent.
- Keep functions small and focused, with clear, descriptive names.
- Handle errors properly, everywhere.
- Remove dead code and unused imports.

## Checks

Run the static checks before opening a pull request. They need Fedora, so on
another system use a container:

```bash
bash scripts/tests/static-check.sh
```

For a Rust app, `cargo check` and `cargo test` in its folder. CI doesn't run
on branches or pull requests, only once work is merged into `main` (and for
release tags), so run the checks yourself before you open the pull request.

## Licences and credit

- Kernel modules are GPL-2.0, everything else GPL-3.0-or-later, vendored code
  in `third-party/` keeps its own licence. Keep the `SPDX-License-Identifier`
  headers.
- VorT2x is a fork of KAIT2EN. Don't change existing authorship: copyright
  lines, people's e-mail addresses, `MODULE_AUTHOR`, old changelog entries,
  or anything in `patches/`. See [NOTICE](NOTICE) and
  [LICENSING.md](LICENSING.md).
- Never say something works on hardware unless it was tested there. Say what
  was compile-tested only.
