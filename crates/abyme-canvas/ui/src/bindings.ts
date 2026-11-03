// Generated from the Rust types of abyme-canvas with ts-rs; do not edit. When the test
// `committed_bindings_match_the_rust_types` fails, replace this file with the text it expects.

export type BoardEntry = { 
/**
 * The board's ID.
 */
id: string, 
/**
 * The title shown for the board, as the board file holds it.
 */
title: string, };

export type BoardView = { 
/**
 * The board's ID.
 */
id: string, 
/**
 * The title shown for the board.
 */
title: string, 
/**
 * The board's mode, if it is a zone. On a mirrored board, the UI offers a
 * drag only on a note.
 */
mode: Mode | null, 
/**
 * One entry per element on the board, in element-ID order.
 */
placements: Array<Placement>, 
/**
 * One entry per relation the board shows, as `read_board` shows them.
 */
relations: Array<Relation>, 
/**
 * The boards above this one, top first, as `overview` places them. It
 * ends where the tree is broken, by a cycle or an owner without a file
 * or a home.
 */
trail: Array<BoardEntry>, };

export type ElementFlag = { 
/**
 * The element's ID.
 */
element: string, 
/**
 * The highest priority among the element's flags.
 */
priority: FlagPriority, };

export type FlagPriority = "task" | "hint";

export type Mode = "free" | "mirrored";

export type Placement = { 
/**
 * The element's ID.
 */
element: string, 
/**
 * The left edge on the board, which for an attached element the core
 * computes from the box it hangs on.
 */
x: number, 
/**
 * The top edge on the board, computed like the left edge.
 */
y: number, 
/**
 * The width, from the board file.
 */
width: number, 
/**
 * The height, from the board file.
 */
height: number, 
/**
 * What the box shows.
 */
shown: Shown, 
/**
 * The boards the element owns, by ID. More than one only after a merge
 * or a hand edit.
 */
inner_boards: Array<BoardEntry>, 
/**
 * Whether the placement is pinned, from the board file.
 */
pinned: boolean, };

export type Relation = { 
/**
 * The element it goes out from.
 */
from: string, 
/**
 * Its kind.
 */
kind: RelationKind, 
/**
 * The element it points to, whose file may be missing.
 */
to: string, 
/**
 * Its label, in full, if it has one.
 */
label: string | null, };

export type RelationKind = "concerns" | "depends_on" | "describes" | "see_also" | "uses";

export type Shown = { "kind": "element", 
/**
 * The semantic type, such as `c4/container`.
 */
type_ref: string, 
/**
 * The name.
 */
name: string, 
/**
 * The one-line summary, if the element has one.
 */
summary: string | null, } | { "kind": "note", 
/**
 * The first line of its text.
 */
name: string, 
/**
 * The rest of its text.
 */
body: string, } | { "kind": "missing" };
