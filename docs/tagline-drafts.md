# Tagline drafts for the live catalog

Drafts of the new `tagline` field for every app live on apps.teamofsilicons.com, for Anil to review. Each is one line of plain text of 60 characters or fewer, the limit the API now enforces.

Source: a read-only `GET https://apps.teamofsilicons.com/v1/apps?limit=100` on 2026-11-17 (30 apps, all published). Nothing was written to production. These apps were published before taglines existed, so they stay published without one; cards fall back to the first sentence of the description until their authors set a tagline, for example:

```bash
silicon-apps setup briefcase details --tagline "A file drive for every Carbon and Silicon account"
```

The last column is what cards show today (the description's first sentence), for comparison.

| App ID | Name | Draft tagline | Chars | First sentence today |
| --- | --- | --- | ---: | --- |
| `diary` | Diary | A private daily diary you keep with invited Silicons | 52 | A private diary for a Carbon and invited Silicons. |
| `lucent` | Lucent | Develop photos through 25 film cameras from the terminal | 56 | Lucent is a film camera for the command line. |
| `omni` | Omni | One conversation across Claude Code, Codex and Antigravity | 58 | Omni gives Claude Code, Codex, and Antigravity one shared conversation. |
| `peek` | Peek | Quick voice-first messages in a small bubble on your Mac | 56 | Peek gives Carbons and Silicons a voice-first way to exchange quick messages on a Mac. |
| `silicon` | Silicon | A local runtime for running your own Silicons | 45 | Silicon is a local agent runtime for Carbons and Silicons. |
| `silicon-accounts` | Silicon Accounts | One account for every Carbon and Silicon | 40 | Silicon Accounts gives every Carbon and Silicon an account for the Silicon ecosystem. |
| `silicon-apps` | Silicon Apps | Publish, install and update command-line apps | 45 | Create, publish, discover and install command-line apps across the Silicon ecosystem. |
| `briefcase` | Silicon Briefcase | A file drive for every Carbon and Silicon account | 49 | Briefcase is the file system for Carbons and Silicons. |
| `browser` | Silicon Browser | Shared, managed browsers for Carbons and Silicons | 49 | Silicon Browser gives people and software agents managed browser profiles, shared access, expiring sessions and live handoff. |
| `calc` | Silicon Calc | Spreadsheets with formulas, comments and full history | 53 | Spreadsheets for Carbons and Silicons. |
| `silicon-cells` | Silicon Cells | An isolated PostgreSQL backend for every project | 48 | Silicon Cells gives every project an isolated PostgreSQL backend. |
| `commit` | Silicon Commit | Todos, projects and shared work for Carbons and Silicons | 56 | Commit is the work manager for Carbons and Silicons. |
| `silicon-contacts` | Silicon Contacts | A personal address book for Carbons and Silicons | 48 | Keep the Carbons and Silicons you know in one personal address book. |
| `silicon-control` | Silicon Control | Battery, hardware and power controls for your Android phone | 59 | Silicon Control gives you a local Android dashboard with a battery gauge, live hardware readings, app restrictions, direct switches and sliders for brightness, screen timeout and media volume. |
| `dm` | Silicon DM | Direct messages and groups for Carbons and Silicons | 51 | DM is the messaging layer for Carbons and Silicons. |
| `extend` | Silicon Extend | Let your Silicons use your phones, computers and TVs | 52 | Extend lets your Silicons use the phones, tablets, computers and TVs you pair with it, the way you use them. |
| `hook` | Silicon Hook | Signed webhook endpoints for every Silicon | 42 | Hook gives every Silicon its own signed webhook endpoints. |
| `mcport` | Silicon MCPort | Set up MCP servers once and call their tools anywhere | 53 | MCPort is the one place to set up MCP servers and use their tools from anywhere. |
| `silicon-organizations` | Silicon Organizations | Work accounts and teams for Carbons and Silicons | 48 | Silicon Organizations brings Carbons and Silicons together in a shared workspace. |
| `present` | Silicon Present | Slides with live HTML, built by Silicons, shown by Carbons | 58 | Silicon Present is slides for Carbons and Silicons: everything you expect from Google Slides or PowerPoint, plus slides that hold live HTML. |
| `remind` | Silicon Remind | Cron-style reminders on a Silicon's own clock | 45 | Remind runs reminders on a Silicon's own clock. |
| `ring` | Silicon Ring | Voice calls to any Carbon or Silicon by their ID | 48 | ring is how a silicon makes and takes voice calls on Silicon Ring. |
| `starter` | Silicon Starter | Find, share and pull reusable starters and components | 53 | Discover, create, and share starters, genes, ISIs and functions in one catalog. |
| `silicon-stream` | Silicon Stream | A realtime relay for text, audio, video and anything else | 57 | Silicon Stream is a realtime relay for Carbons, Silicons and apps. |
| `ting` | Silicon Ting | Durable notifications for Carbons and Silicons | 46 | Silicon Ting delivers durable notifications to Carbons and Silicons. |
| `waveform` | Silicon Waveform | Text to speech and speech to text across providers | 50 | Waveform turns text into speech and speech into text for Carbons, Silicons and the apps they use. |
| `write` | Silicon Write | Shared documents with comments, suggestions and history | 55 | Silicon Write is a shared home for documents, for Carbons and Silicons. |
| `silicon-writings` | Silicon Writings | Write and publish articles and notes together | 45 | Silicon Writings is where carbons and silicons at Team of Silicons write articles and notes together and publish them to teamofsilicons.com/writings. |
| `spacestation` | Space Station | Record events, query them with SQL and watch them live | 54 | Space Station lets Carbons and Silicons record application events, run read-only SQL, build live Space Windows, and receive notifications. |
| `spotify` | Spotify | Control Spotify playback from the command line | 46 | Control Spotify from the command line as a Carbon or Silicon. |

Longest draft: 59 characters. Shortest: 40.
