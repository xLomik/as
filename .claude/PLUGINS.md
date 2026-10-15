# Plugins / skills de Claude Code en este repo

Declarados en `.claude/settings.json` (`extraKnownMarketplaces` + `enabledPlugins`).
Al abrir el repo con Claude Code y confiar en la carpeta, se instalan solos.
Instalación manual alternativa: `/plugin marketplace add <owner/repo>` y luego `/plugin install <plugin>@<marketplace>`.

| Plugin | Marketplace (repo) | Para qué |
|--------|--------------------|----------|
| `superpowers` | `obra/superpowers` | Flujo de trabajo: brainstorming → plan → TDD → debugging sistemático → code review → verificación |
| `example-skills` | `anthropics/skills` | Skills oficiales de Anthropic: skill-creator, mcp-builder, frontend-design, webapp-testing, etc. |
| `swiftui-expert` | `AvdLee/SwiftUI-Agent-Skill` | SwiftUI moderno: estado, composición, rendimiento, APIs nuevas del SDK |
| `swiftui-pro` | `twostraws/SwiftUI-Agent-Skill` | Revisión de código SwiftUI (Paul Hudson) |
| `swift-concurrency` | `AvdLee/Swift-Concurrency-Agent-Skill` | async/await, actors, Sendable, Swift 6 (clave para AVFoundation/cámara) |

Ya vendorizada aparte: `caveman` (ver `.claude/skills/caveman/SOURCE.md`).
