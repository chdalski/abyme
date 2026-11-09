---
tags:
  - meta
---
# Ideas

Collected ideas that have not been discussed yet. When an idea is adopted, it moves into the matching note and leaves this list.

## Visual Collaboration with the AI

- **Selection as context:** The developer selects elements in the canvas and asks the AI about them, such as “What depends on this?” or “Split this up”. The AI fetches the current selection through MCP, e.g. with `get_selection`.
- **Seeing AI changes live:** The canvas shows the AI's changes right away and highlights what it just changed. You can watch it work and step in directly.
- **Showing instead of describing:** The AI can point at elements in the canvas, that is highlight them or zoom to them, while it explains something, e.g. with `focus`.
- **Tasks on the board:** A sticky note to the AI, such as “@ai the queue is missing here”, lands in its status on the next call and is handled there.

## Team Collaboration

The core scenario is one developer with an AI; teams collaborate through Git (see [[Vision#Audience]]). Even that means two participants, and more with several AI sessions. What they need also helps when more people join.

### Through Git

- **Board diff and check in the pull request:** `abyme diff main..feature` draws an image of the board through `abyme-render`, with new, removed, and changed parts marked in color. A CI action attaches it to the pull request as a comment. On top, `abyme check` reports the representations whose anchors the pull request changes without them having been updated. This works without the AI, because anchors, verification stamps, and `git diff` are enough. Reviewers thus see how the architecture changes and whether the docs kept up.
- **Merge driver:** A Git merge driver for board files resolves conflicts in positions itself and reports only real contradictions, for example when one person deletes an element the other one moves.
- **Discussions on elements:** Sticky notes get replies, mentions such as `@anna` or `@ai`, and can be marked as done. They live in Git like everything else.
- **Owners through anchors:** The anchor of a representation is a path, and for paths `CODEOWNERS` knows the owners. Review flags and change requests thus go to the right person.
- **Provenance:** Every element knows who created it (person or AI, which model, which session), when, and on what basis. On top comes a status “confirmed by a person”. This protects the shared truth from content the AI made up.
- **AI changes as proposals:** Per zone, you can set whether the AI writes directly or makes a proposal that a person accepts, similar to a pull request.
- **Tours:** a stored sequence of board sections with text, such as “How an order travels through the system”, for new team members. The AI can create them. When the elements shown change, the tour gets a review flag.

### Sessions as Participants

- **Presence of the AI:** Every AI session is visible on the board, with its name, the area it works in, and the elements it changed last. Abyme recognizes the session by its MCP connection.
- **Claiming work:** A session or a person claims a zone or a change request, and the others see “in progress”. Several agents thus work in parallel without getting in each other's way, and the board becomes a control room. The model is the blackboard architecture, in which specialists cooperate through a shared board.
- **Handover:** When a session ends, it leaves its state on the board. A person or another session continues there.
- **Working and long-term memory:** Every AI session gets a note board. At the end, the AI moves the relevant findings into the permanent boards, and the rest expires.

### Terminals (Later)

- **Terminal next to the board:** A terminal, running for example Claude Code connected to Abyme, opens in its own tab or in a split view next to the board. That way it does not zoom along. Technically this is xterm.js in the UI and a PTY on the Rust side, for example through `portable-pty`.
- **What it brings:** Abyme knows which session belongs to which terminal. On the board, its presence shows where it works, and tasks on sticky notes go straight to it. Without a terminal, Abyme cannot prompt the AI on its own; it waits for the AI's next call.
- **What is not in Git:** the content of the terminal
- **Risk:** A shared terminal in a real-time session would mean that others run commands on your own machine. That would only work with explicit consent.

### In Real Time

- **Several people on one board:** seeing the others' cursors and selections, and following someone else's view, for example to explain the architecture in a meeting.
- **The way there:** This needs [[Hexagonal Architecture#Runtime|stage B]] with `abyme serve`, reachable over the network, plus sign-in and roles and permissions. The [[Data Model#Operations|operations]] are already data. The server orders them and checks modes and rules. Cursors, selection, and camera run over an ephemeral channel and never land in Git.
- **CRDTs** such as Yjs (in Rust `yrs`), Automerge, or Loro only pay off when people work offline and merge later. For that there is already Git.
- **React Flow:** The multiplayer guide is free; the ready-made example with Yjs is a Pro example.

## Specifications

In spec-driven development, a specification comes before the code: requirements with acceptance criteria, a design, and tasks. The AI then implements it. Abyme already has much of this:

- **Specification as target:** Requirements and design lie in target frames. The [[Current and Target#Diff|diff]] shows what is new and what changes. What already exists is in the current state and is not mistaken for something new.
- **A specification that stays:** After implementation, target elements become representations (see [[Elements#From Draft to Representation]]). The specification turns into current docs with anchors and checks instead of being left lying around after the feature.
- **Type pack “specification”:** types such as requirement, acceptance criterion, and task; relations such as “fulfills” (design → requirement) and “tests” (test → criterion); plus type rules such as “every requirement is fulfilled by at least one design element” or “every criterion has a test”. These become structural [[Checks]] that find gaps in the specification deterministically.
- **Traceability:** requirement → design → current representation → code, and criterion → test representation → test file. When code or a test changes, the requirement is flagged through the [[Reconciliation#The Relation Chain|relation chain]].
- **Review on the picture:** Instead of many Markdown files, you see the target with the diff and the traceability. The texts live in the element files and are normal text diffs in the pull request.
- **Size as needed:** a sticky note for a small bug, a target frame for a feature, a target board with inner boards for a large undertaking. There is no fixed workflow. Whoever wants a guided workflow gets it as a skill and a zone preset.
- **Existing specifications:** If specifications already lie in the repo as Markdown, they become representations in a mirrored zone. Abyme shows and checks them without replacing them.
- **Tasks** do not have to live in Abyme (see [[Current and Target#Plans]]). They can point to a task list or to tickets as representations.
- **Not a goal:** generating code from the specification alone, without ever touching it. The code is written by the user's AI.

## Finding Your Way Through the Boards

- **Folders for boards:** Boards are sorted into folders that need no element, as notes are in Obsidian, such as a folder “Team” with a board “Onboarding”. Today a board lies below another only when an element owns it (see [[Board Graph]]); folders let a person build a hierarchy that stands for nothing outside, without inventing an element for each level.
- **Graph view of the boards:** One picture of all boards, each a node, with the inner-board hierarchy and the relations between elements on different boards summed up per pair of boards, such as “6 × describes” from a docs board to a components board. It shows how the boards hang together at a glance, also where no board draws those relations because they cross board boundaries (see [[Board Graph#Relations Across Boards]]). Elements stay out, since a graph of every element would be too crowded to read.
- **Animated data flows:** Arrows that stand for a flow of data animate along their direction, and a flow can play step by step like a sequence diagram. A reader sees how a request travels through the system instead of reading it from labels. It needs a way to tell which relations are flows and what flows along them, which today's relation kinds do not say (see [[Relations]]).

## More

- **Navigation instead of a dump:** MCP tools such as `look`, `enter`, `zoom_in`, and `follow_link`. The AI moves through the boards like through a text adventure and loads only what it needs.
- **Boards from queries:** dynamic boards such as “all components that depend on DB X”, similar to Dataview in Obsidian.
- **Time travel:** “What did the architecture look like in March?” comes almost for free from the Git history.
- **Abyme describes itself:** As soon as it runs, the Abyme repo gets its own `abyme/` with its own architecture, as the first real test case.
- **Relations as Obsidian links:** If the targets of relations were in the frontmatter as `[[…]]`, Obsidian would show the graph of the elements for free (see [[Storage in Git#Element File]]).
- **Board images in the docs:** Markdown files in the repo, such as the README, embed an image of a board. A hook or the CI redraws it when the board changes.
- **Conventions for the AI:** short rules in plain language, such as “services are named like their crate” or “flows run left to right”. The MCP server passes them along with writing calls. Whatever can be checked is better turned into a type rule or default presentation in the [[Extensions#Type Packs|type pack]]: consistency by construction is more reliable than by instruction.
- **Intermediate steps in the target:** mark target elements with plan steps (e.g. task 2, task 5) and filter by them, see [[Current and Target#Plans]].

## Deferred

Not in focus (see [[Vision#Audience]]):

- **Live data**, that is sources with continuously changing values such as price charts. This would include telling changes apart as “deviation” and “routine”, and values that are not in Git but fetched when displayed.
- **Conditions on relations:** A relation triggers a check only once a condition occurs, for example “as soon as the price falls below X”. This makes sense mainly with live data.
