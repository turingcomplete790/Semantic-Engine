# Feature Specification: Engine Plugin Foundation

**Feature Branch**: `002-engine-plugin-foundation`

**Created**: 2026-10-15

**Status**: Draft

**Input**: User description: "Phase 0" (the remaining foundation items of `ROADMAP.md` Phase 0:
workspace, plugin manifest with a no-op operation, server identity task, API document, fixture
tooling, and CI with prebuilt binaries).

## Overview

The vocabulary (feature 001) is done. Phase 0 is not finished until the project can be built,
installed into a Stash server as a plugin, invoked from Stash, and described to the two client
projects. This feature delivers that foundation without any semantic behaviour yet: an installable
plugin that answers a "ping" and a "status" operation, a one-time server identity task, a written
API document for clients, tooling to capture test fixtures from a library, and automated builds.

The Semantic Stash Viewer and the semantic-tagging plugin are the consumers. Everything built here
is the base the spike (Phase 1) and the graph (Phase 2) stand on.

## Clarifications

### Session 2026-10-15

- Q: How is the plugin installed, and on which platforms? → A: Two install scripts cover all of
  Stash's platforms: a POSIX shell script that runs on BSD, macOS, and Linux without
  bash-specific features, and a PowerShell script for Windows.
- Q: What should an install script do when there is no prebuilt package for the owner's
  platform? → A: Stop before changing anything, naming the detected platform, where packages are
  published, and how to build from source; it never builds from source itself.
- Q: How is RocksDB (the long-term cache store) provided? → A: Bundled: Oxigraph's default build
  compiles RocksDB from source with a plain Cargo build (no CMake), so packages are
  self-contained and owners install nothing. This replaces an earlier answer (an installed system
  package with a pinned version) after a probe showed Oxigraph 0.4 does not build against an
  installed RocksDB 11.1.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Install the plugin and see it respond (Priority: P1)

A Stash owner installs the engine plugin into their server and, from Stash itself, invokes a
status operation and gets a correct answer.

**Why this priority**: Nothing else can be tested until a build of the plugin runs inside Stash and
Stash can call it, and the install must work the same way on every platform Stash supports.

**Independent Test**: Run the install script for the platform (POSIX shell on BSD, macOS, or
Linux; PowerShell on Windows) against a Stash server's plugins folder, reload plugins, call the
status operation through Stash, and receive the engine version, vocabulary version, and contract
version.

**Acceptance Scenarios**:

1. **Given** the owner runs the install script for their platform, **When** it finishes and the
   owner reloads plugins, **Then** Stash lists the plugin with its tasks and no manifest error.
2. **Given** the plugin is loaded, **When** a client calls the status operation, **Then** the
   response names the engine version, the vocabulary version, and the operation contract version.
3. **Given** the plugin is loaded, **When** a client calls an unknown operation, **Then** it gets a
   structured error naming the operation, not a crash or empty reply.
4. **Given** the plugin is already installed, **When** the owner runs the install script again,
   **Then** it replaces the installed files with the package's files and leaves the server's
   stored server identifier and plugin settings untouched.
5. **Given** a platform or plugins folder the script cannot use, **When** it runs, **Then** it
   stops before changing anything and says why.

---

### User Story 2 - One identifier per server (Priority: P1)

A user runs a task once and the server gets a permanent identifier that every client reads
identically.

**Why this priority**: The vocabulary's item identifiers (FR-014 of feature 001) cannot be built
until this exists, and two clients must agree on it.

**Independent Test**: Run the identity task on a server with none; read the value back through
Stash; run the task again and confirm the value did not change; read it from a second client
connection and confirm it matches.

**Acceptance Scenarios**:

1. **Given** a server with no identifier, **When** the owner runs the identity task, **Then** a
   new identifier is generated and stored in Stash.
2. **Given** an existing identifier, **When** the task runs again, **Then** the identifier is
   unchanged and the task reports that it already existed.
3. **Given** the identifier is stored, **When** any client reads it through Stash, **Then** it is
   a lower-case hyphenated UUID, identical for every client.
4. **Given** no identifier exists, **When** a client asks for it, **Then** the engine reports "not
   generated yet" and does not invent one.

---

### User Story 3 - A written API document clients can build against (Priority: P2)

A developer on the viewer or the plugin can read one document that states every operation, its
request and response shapes, its errors, and its versions.

**Why this priority**: Both client projects want to stop building their own engines; they need the
contract before the engine has features.

**Independent Test**: A developer who has not seen the engine implements a client call to the
status operation from the document alone and it works against an installed plugin.

**Acceptance Scenarios**:

1. **Given** the API document, **When** a developer looks up an operation, **Then** they find its
   name, purpose, request fields, response fields, error codes, and the version it appeared in.
2. **Given** an operation changes, **When** the change is released, **Then** the document and the
   contract version change in the same revision.
3. **Given** a term from the vocabulary appears in the document, **When** the reader follows it,
   **Then** it is referred to by its vocabulary identifier, not redefined.

---

### User Story 4 - Repeatable builds with downloadable binaries (Priority: P2)

A maintainer pushes a change and gets formatted-and-tested results; a tagged release produces
installable plugin packages for Linux.

**Why this priority**: The plugin must be installable by users who do not have a Rust toolchain,
and regressions must be caught early.

**Independent Test**: Push a change that breaks formatting; the build fails. Tag a release; a
package appears containing the manifest and executable that installs per User Story 1.

**Acceptance Scenarios**:

1. **Given** a change, **When** the automated build runs, **Then** formatting, lint, tests, the
   vocabulary check, and the second-parser check all run and any failure fails the build.
2. **Given** a release tag, **When** the release build finishes, **Then** it produces a package
   with the manifest, executable, and vocabulary version recorded.
3. **Given** the package, **When** it is unpacked into a plugins folder, **Then** it satisfies
   User Story 1 without further steps.

---

### User Story 5 - Capture test fixtures from a library without changing it (Priority: P3)

A developer captures read-only responses from a tagged library to use as fixtures for later
features.

**Why this priority**: Later features (graph build, queries) are test-first and need real data
without a live server.

**Independent Test**: Point the capture tool at a test server and get a set of fixture files;
confirm the server's data is unchanged and the files contain no credentials.

**Acceptance Scenarios**:

1. **Given** a server address and API key, **When** the capture tool runs, **Then** it writes
   response files for the semantic vocabulary queries and only issues read operations.
2. **Given** the captured files, **When** they are inspected, **Then** no API key, cookie, or
   other credential appears in them.
3. **Given** a server with no semantic data, **When** the tool runs, **Then** it still completes
   and says which parts were empty.

---

### Edge Cases

- Stash starts the plugin with no connection details or a malformed request: the plugin returns a
  structured error and exits cleanly.
- Two identity tasks run at once: only one identifier is stored and both report the same value.
- The server's stored identifier is malformed (edited by hand): the engine reports it as invalid
  and does not overwrite it silently.
- The plugin binary is for the wrong platform or architecture, or no prebuilt package exists for
  it (for example a BSD): the install script detects the operating system and architecture, and
  refuses with a message that names what it found and how to build from source.
- The install script is run without write access to the plugins folder, or the folder cannot be
  found: it stops before changing anything and says how to point it at the right folder.
- The API document and the implemented operations disagree: a check fails the build.
- The capture tool is pointed at a server it cannot reach or authenticate to: it fails with a
  clear message and writes nothing.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The repository MUST build as a workspace containing the vocabulary crate, a Stash
  adapter crate, an engine library crate, and a plugin executable crate, each with a stated
  responsibility and no dependency on a UI toolkit.
- **FR-002**: The plugin MUST ship a Stash manifest declaring its name, version, executable,
  tasks, and the operation interface Stash uses, and MUST load in Stash without manifest errors.
- **FR-003**: The plugin MUST implement a **status** operation returning the engine version, the
  vocabulary version, the operation contract version, and the plugin's build information.
- **FR-004**: The plugin MUST implement a **ping** operation that returns without side effects
  and is usable to measure invocation cost.
- **FR-005**: Every operation MUST return structured responses: success with named fields, or an
  error with a code, a message, and the operation name. An unknown operation MUST return an error
  of its own code.
- **FR-006**: The plugin MUST implement a **generate server identifier** task that stores a
  lower-case hyphenated UUID in Stash once per server, is idempotent, and reports whether it
  generated or found one.
- **FR-007**: The plugin MUST implement a **read server identifier** operation returning the
  stored identifier, or a "not generated" error; it MUST NOT generate one implicitly.
- **FR-008**: The server identifier MUST be readable by any client through Stash without the
  engine installed, so a client that cannot use the engine can still build identical item
  identifiers.
- **FR-009**: The Stash adapter MUST be the only code that communicates with Stash; it MUST use
  the GraphQL client library mandated by the constitution with typed operations generated
  against a pinned copy of Stash's schema, and MUST select only the fields each caller needs.
- **FR-010**: An **API document** MUST list every operation with name, purpose, request fields,
  response fields, error codes, and the contract version it appeared in, and MUST refer to
  vocabulary terms by identifier.
- **FR-011**: The operation contract MUST carry an explicit version, returned by the status
  operation; a breaking change MUST change its MAJOR number, and the API document MUST record
  the change.
- **FR-012**: An automated check MUST confirm that the operations in the API document and the
  operations the plugin implements are the same set, with matching error codes.
- **FR-013**: Automated builds MUST run formatting, lint, tests, the vocabulary check, and the
  second-parser check on every change, and MUST fail on any failure.
- **FR-014**: A release build MUST produce installable packages for each platform Stash
  supports that the toolchain can target (Linux, macOS, and Windows, each by architecture),
  each containing the manifest, the executable, and the vocabulary files, and MUST state the
  versions it contains.
- **FR-015**: A **fixture capture tool** MUST record read-only responses for the semantic data
  (flagged tags, assignments, pairings, timelines, label formulas, variations, archetypes) from a
  configured server into fixture files, MUST issue only read operations, and MUST NOT write any
  credential to disk.
- **FR-016**: The plugin MUST NOT open a network listener, MUST NOT contact any host except the
  Stash server that invoked it, and MUST NOT log credentials (constitution Principle IV).
- **FR-017**: The plugin MUST work against Stash v0.31.1 or newer and MUST report a clear error
  when the server's version is older than the minimum.
- **FR-018**: The project MUST provide an install script for BSD, macOS, and Linux written in
  POSIX shell (no bash-specific features, and only utilities present on all three), and an
  install script for Windows written in PowerShell.
- **FR-019**: Each install script MUST detect the operating system and architecture, locate or
  accept the Stash plugins folder, place the plugin files there, and report what it installed. It
  MUST be safe to run repeatedly, MUST NOT modify anything outside the plugins folder, and MUST
  stop with a clear message before changing anything when it cannot proceed. When no prebuilt
  package exists for the detected platform and architecture, it MUST stop, naming the detected
  platform, where packages are published, and how to build from source, and MUST change nothing.
- **FR-020**: The install scripts MUST NOT require the Rust toolchain on the owner's machine and
  MUST NOT send any data anywhere except to download the package from its published location.
- **FR-021**: The engine's on-disk store MUST use the RocksDB bundled with the storage library
  and compiled as part of the normal build, with no CMake and no system RocksDB. Packages MUST be
  self-contained: an owner installs nothing besides the plugin.
- **FR-022**: Building from source MUST need only the Rust toolchain, a C++17 compiler, and clang,
  and every build-from-source instruction MUST list them. The status operation MUST report the
  bundled RocksDB version.

### Key Entities

- **Plugin package**: the manifest, executable, and vocabulary files that together are installed
  into a server's plugins folder.
- **Operation**: a named request the plugin answers when invoked through Stash, with a request
  shape, a response shape, errors, and a contract version.
- **Task**: a named action the plugin runs on demand from Stash (for example generate the server
  identifier), reporting progress and a result.
- **Server identifier**: a UUID stored once per server in Stash, read identically by all clients.
- **Contract version**: the version of the set of operations and their shapes.
- **Fixture**: a recorded, credential-free set of read-only server responses used by tests.
- **API document**: the written description of the operations and their versions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An owner can install the plugin with the install script for their platform and see
  a correct status response from Stash in under 5 minutes, using only the script and Stash.
- **SC-002**: Running the identity task twice yields the same identifier 100% of the time, and
  two clients reading it get identical values.
- **SC-003**: A developer who has not seen the engine can implement a status call from the API
  document alone on the first attempt, verified with at least one independent reader.
- **SC-004**: Every change is checked by the automated build, and a deliberately broken change in
  each of formatting, lint, tests, vocabulary rules, and operation/document mismatch fails it.
- **SC-005**: A status response arrives in under 1 second on a local server, measured and
  recorded as the baseline for the Phase 1 spike.
- **SC-006**: Captured fixtures contain no credential strings and the capture leaves the server's
  data unchanged, verified by comparing counts before and after.
- **SC-007**: The POSIX shell install script runs without modification on a BSD, a macOS, and a
  Linux system, verified by running it under a strict POSIX shell on each, and the PowerShell
  script is verified on Windows.

## Assumptions

- The Stash raw plugin interface is used (the server runs the executable per invocation with
  JSON in and out); whether a long-lived process is better is a Phase 1 spike question and is
  out of scope here.
- The server identifier is stored as a plugin setting of this plugin in Stash configuration; the
  exact setting name is fixed in this feature's plan and listed in the API document and the
  vocabulary mapping.
- Packages are produced for the platforms the toolchain can target in automated builds; a
  platform with no prebuilt package is reported by the install script, not silently skipped.
- The API document is Markdown with JSON examples, kept in the repository and versioned with it.
- The engine's long-term cache is stored with the bundled RocksDB; this feature only builds and
  links it, and the status operation reports its version. Using it as the graph store is Phase 2.
- Semantic behaviour (graph build, queries, hooks, incremental updates) is out of scope; those
  belong to Phases 1 to 3.
- The viewer's and the plugin's own adoption of the engine is out of scope (Phase 4).
