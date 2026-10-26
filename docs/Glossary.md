---
tags:
  - meta
---
# Glossary

The docs, the UI, and the code use these terms alike. The type names are in [[Data Model#Types]].

## Boards and Elements

| Term | Meaning |
|---|---|
| [[Boards and Zones\|Board]] | infinite surface, view and working medium |
| [[Boards and Zones\|Frame]] | named area on a board |
| [[Boards and Zones\|Zone]] | board or frame with settings that are inherited |
| [[Boards and Zones#Zone Presets\|Zone preset]] | predefined combination of mode and statement kind |
| [[Boards and Zones#Modes\|Mode]] | free / synced / mirrored / locked |
| [[Boards and Zones#Statement Kind\|Statement kind]] | current / target / decision / idea / knowledge / question / historic |
| [[Elements\|Element]] | something with a stable identity on a board |
| [[Elements#Native or Representation\|Native element]] | exists only on the board |
| [[Elements#Native or Representation\|Representation]] | stands for an external resource |
| [[Elements#Anchors\|Anchor]] | URI that connects a representation or a zone with an external resource |
| [[Board Graph]] | all boards and elements with references, inner boards, and relations; lives in the core |
| [[Board Graph\|Home]] | board or frame an element lies on |
| [[Board Graph\|Reference]] | display of an element outside its home |
| [[Board Graph\|Inner board]] | the inside of an element, reached by zooming in |
| [[Board Graph#Relations Across Boards\|Stub]] | short arrow for a relation to an element that is not on the board |
| [[Board Graph#Relations Across Boards\|Implied relation]] | arrow on the parent board derived from an inner board, not stored |
| [[Relations\|Relation]] | typed edge from one element to another, with an optional label |
| [[Relations\|Relation kind]] | concerns / depends_on / describes / see_also / uses; later contains / corresponds_to / replaces |
| [[First Slice#MVP\|Docs map]] | the Markdown documents of a repo as document representations, with relations to the code they describe |
| [[Presentation]] | how an element appears on the board: box, card, note, document, image, chart, table |
| [[Presentation#Sticky Notes\|Sticky note]] | native element with the presentation note, free or attached |
| [[Presentation#Sticky Notes\|Attach]] | hang an element on another one so it moves along; layout, not content |
| [[Data Model\|Placement]] | where and how an element lies on a board: rectangle, presentation, home or reference |
| [[Data Model#Rules\|Summary]] | short text on an element or board, usable by every output |
| [[MCP Server#Detail Levels\|Detail level]] | how much the AI reads at once: overview, board outline, or element detail |
| [[MCP Server#Detail Levels\|Board outline]] | a board as compact text: its elements and the relations between them, one per line |
| [[MCP Server#Detail Levels\|Element detail]] | one element as text: its fields, boards, relations in both directions, and description |

## Reconciliation

| Term | Meaning |
|---|---|
| [[Reconciliation#Direction of Changes\|Base snapshot]] | state at the last reconciliation, basis for detecting the direction |
| [[Change Requests\|Change request]] | change on the board that is to be implemented in the external resource |
| [[Checks\|Review flag]] | “this needs to be reviewed” |
| [[Checks#Verification Stamp\|Verification stamp]] | “last verified against commit X” |
| [[Current and Target\|Correspondence link]] | connection between a target element and a current element |
| [[Current and Target#Diff\|Diff]] | difference between target and current |

## Architecture

| Term | Meaning |
|---|---|
| [[Hourglass]] | architecture: many inputs and outputs, connected through a narrow waist |
| [[Data Model\|Narrow waist]] | the Abyme format in Git, shared by inputs and outputs |
| [[Hourglass#Inputs\|Input]] | how content gets into Abyme, above all through the AI via MCP |
| [[Hourglass#Outputs\|Output]] | where Abyme is shown; the own canvas is full-featured, other systems get exports |
| [[Extensions\|Extension]] | type pack, export, parser, or fetcher |
| [[Extensions#Type Packs\|Type pack]] | semantic types, relation types, rules, and templates, e.g. C4 |
| [[Extensions#Parsers\|Parser]] | reads known formats in the repo without the AI, optional |
| [[Extensions#Code Parser\|Code parser]] | parser based on tree-sitter, reads definitions and references in the code |
| [[Extensions#Fetchers\|Fetcher]] | fetches data from sources outside the repo, e.g. tickets (later) |
| [[Extensions#Parsers\|Snapshot]] | normalized state of a source, delivered by a parser or fetcher |
| [[Hexagonal Architecture\|Port]] | interface the core defines; the input port for callers, output ports for what the core itself needs |
| [[Hexagonal Architecture\|Driving adapter]] | calls the core through the input port, e.g. canvas or MCP server |
| [[Hexagonal Architecture\|Driven adapter]] | is called by the core through an output port, e.g. storage or search index |
| [[Hexagonal Architecture#Core API\|Core API]] | the core's input port; for now the public API of `abyme-core`, from stage B a trait (`AbymeApi`) usable embedded or remote |
| [[Data Model#Operations\|Operation]] | a change held as data, such as creating, moving, or pinning an element; applying it returns the file writes |
| [[Data Model#Operations\|Local key]] | a name for something a request creates, valid only within that request |
| [[Canvas#Projecting a Board\|Projection]] | a board from the board graph, translated into what an interface displays |
| [[Canvas#Visual Check\|Visual check]] | the AI fetches an image of the board to check its result |
| [[Canvas#Visual Check\|Scene]] | shapes, geometry, style, and text of a board, from which the render tool draws |
| [[Search and Index\|Search index]] | LanceDB table in the user's cache that ranks a project's elements for `search`; derived, never the source of data |
| [[Search and Index#Embedding Model\|Embedding model]] | model that turns element text into vectors for search; each project picks one of the models built into Abyme |
