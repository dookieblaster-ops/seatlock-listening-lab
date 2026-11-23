# How to maintain this wiki

| Field | Value |
| --- | --- |
| **Kind** | meta |
| **Updated** | 2026-11-23 |
| **Status** | living |

## Purpose

Keep a **living**, **parsable** map of the software. The README remains the specification. The wiki remains the “what is this folder / crate for?” guide.

## Rules

1. **One job per article.** Prefer a new page over a mega-page.
2. **Keep the table at the top.** Every article starts with the Kind / Summary / Code / Spec / Updated / Status table so tools and humans can scan it.
3. **Update `Updated` when you change meaning**, not for typo-only edits.
4. **Link code paths that exist today.** If you rename a crate or module, update the wiki in the same commit.
5. **Do not copy the README.** Summarize and link section numbers for physics, checks, and product rules.
6. **Do not invent behavior.** If you are unsure, read the code or STATUS; leave a “Status: stub” note rather than guessing.
7. **Add a Home row** when you add an article.
8. **After a session that changes architecture**, update the affected wiki pages before the handoff (same rule as STATUS / COMS).

## Parsable conventions

- Headings: `## Overview`, `## Responsibilities`, `## How it works`, `## Modules and types`, `## Inputs and outputs`, `## Checks`, `## See also` in that order when they apply.
- Code: backticks for paths and symbols.
- Cross-links: relative markdown links inside `docs/wiki/`.
- New articles: copy [`_ARTICLE_TEMPLATE.md`](_ARTICLE_TEMPLATE.md).

## What not to put here

- Session handoffs → `COMS.md`
- Decision history → `STATUS.md`
- Long derivations and CHK tables → `README.md` §5 / §13
- Owner backup clicks → `docs/backups.md`

## See also

- [Home](Home.md)
- [Architecture](Architecture.md)
