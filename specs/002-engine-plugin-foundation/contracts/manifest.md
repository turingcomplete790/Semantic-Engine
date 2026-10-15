# Contract: Plugin Manifest

File `semantic-engine.yml` at the root of the installed plugin folder; plugin id
`semantic-engine` (Stash derives it from the file name).

```yaml
name: Semantic Engine
description: Shared semantic engine for the Semantic Stash Viewer and semantic-tagging.
url: <project URL>
version: <engine version>
exec:
  - semantic-engine          # resolved relative to the plugin folder; .exe optional on Windows
interface: raw
errLog: error
tasks:
  - name: Generate server identifier
    description: Create this server's permanent identifier if it does not exist yet.
    defaultArgs:
      op: generateServerId
settings:
  serverId:
    displayName: Server identifier
    description: Generated once by the "Generate server identifier" task. Do not edit.
    type: STRING
```

Rules: no `ui` section (no JavaScript is injected); no hooks in this feature (Phase 3); the
manifest version equals the engine version.
