# Quill

Quill is a macOS menu bar notes app, forked from [Quiet.md](https://github.com/s1dashu/quiet-md) (Rust + GPUI, MIT) with every Quiet.md feature kept. It runs from the menu bar by default and can still run as a standard windowed app. Opening the menu bar panel shows the Quiet.md home view with a "new note" field on top; focusing that field expands it into a new entry while the home view slides out the bottom. The upstream code conventions (compact style, no rustfmt, `fdl_core` boundary, DocxXML stability, user data rules) still apply: see [`docs/upstream/CLAUDE.md`](docs/upstream/CLAUDE.md) and [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). Upstream is the `upstream` git remote.

---

## 1. Communication & output style

Prose (chat replies, commit messages, PR text, markdown, comments):
- **No em dashes.** Rewrite the sentence with a comma, semicolon, colon, period, or parentheses instead of an `—`. This targets em dashes and their look-alikes (`--`, spaced `-`, stray en dashes), not real hyphens: leave compound adjectives (`well-known`), ranges (`10-20`, `2019-2024`), and anything in code, paths, or URLs alone.
- Prefer plain ASCII punctuation and straight quotes over smart quotes and other Unicode decoration.
- No emojis unless the user asks for them.
- Skip filler openers ("Sure!", "Great question") and hollow closings ("Hope this helps!", "Let me know if..."). Answer directly and do not restate the request back.
- Be concise: short, direct statements over verbose explanation. Say "I don't know" rather than guess; do not invent APIs, files, or flags.

## 2. Code & scope

- Stay within the requested scope. No unsolicited refactors, renames, or "while I'm here" changes to code outside the task.
- Do not add or upgrade dependencies without listing them and asking first.
- Do not delete existing comments or code unrelated to the change.
- Prefer the simplest working solution; do not over-engineer or add abstraction that wasn't asked for.
- Treat user corrections as authoritative for the rest of the session.

## 3. Repo etiquette

- Never touch real user data: the default Quiet.md library (`~/Documents/Quiet.md`) and Quill's own library. Test libraries go in a temp dir via the home env var.

- Branch off `main`; land changes via pull request (do not push to `main`).
- Match the style of the code you're editing.
- Verify a change by running it (tests, the app, the browser), not by reading it.
- UI design changes are drawn first, then built: draw them in Spill with the spill_design tools on the spec.

## 4. Specs live in Spill

Every question, spec, and decision for this project is tracked in **Spill**, the visual spec board.
The user watches the board to see what is being asked, approved, and worked on, so the board must
always reflect reality. Full instructions: [`.spill/SPILL.md`](.spill/SPILL.md). Read it at the
start of every session that touches product behavior.

The short version:
- **Most important: fetch and update spec state.** Check the board before each step, and never
  leave a spec `active` when you are not building it. When you stop for any reason (done, waiting
  on the user, blocked, switching away), move it in the same step: `done`, a question that blocks
  it, or back to `discussing`. Check every active spec before ending a turn.
- **Specs before code.** No feature code without an `approved` spec. Draft it on the board first.
- **Build means build.** Approved is not a go. When the user presses Build (the spec is `queued`),
  start building it without waiting to be asked (one active spec at a time, in the order queued).
- **Too large? Recommend a split.** At any point before a spec is done, if it has grown too large
  to build and check as one piece, ask on the board to break it into smaller specs, naming them.
  A split is a reorganization: each part is a complete spec that owns its questions and decisions
  (moved from the original), and the original is rewritten as the overview.
- **Questions go on the board, not only in chat.** Every question you need answered becomes a
  `Q-n` card. Check the board for answers (and for questions the user added) before asking again.
- **Move cards as you work.** Set a spec `active` when you start it and `done` when it ships.
  Only one spec is `active` per session unless the user says otherwise.
- **An answer that picks none of the options is a redirect.** Read it and act on what it means:
  an answer of the user's own is recorded as the decision; "this shouldn't be a question" drops the
  question (comment why); only an unclear question is rewritten and reopened. Never open a new
  question for the same thing.
- **Never approve or reject your own cards.** Only the user moves a card to `approved`, `queued` or `rejected`
  (the board can also auto-approve; that is not you).
- If the Spill MCP server is not connected, use the file fallback in `.spill/` described in
  `SPILL.md`, and say so once.
