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
 * The board's mode, if it is a zone. The UI does not offer a drag on a
 * mirrored board.
 */
mode: Mode | null, 
/**
 * One entry per element on the board, in element-ID order.
 */
placements: Array<Placement>, };

export type Mode = "free" | "mirrored";

export type Placement = { 
/**
 * The element's ID.
 */
element: string, 
/**
 * The left edge, from the board file.
 */
x: number, 
/**
 * The top edge, from the board file.
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
shown: Shown, };

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
summary: string | null, } | { "kind": "missing" };
