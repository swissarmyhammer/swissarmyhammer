---
title: C# / .NET Project Guidelines
description: Best practices and tooling for .NET projects
partial: true
---

### C# / .NET Project Guidelines

**Testing — do NOT glob; .NET auto-discovers test projects:**
- All: `dotnet test`
- Verbose: `dotnet test --verbosity normal`
- Filter: `dotnet test --filter FullyQualifiedName~TestMethodName`
- Coverage: `dotnet test --collect:"XPlat Code Coverage"`

**Common commands:**
- Restore: `dotnet restore`
- Build: `dotnet build` (configure with `-c Debug`/`-c Release`)
- Run: `dotnet run`
- Clean: `dotnet clean`
- Publish: `dotnet publish -c Release`
- Add package: `dotnet add package <Name>`

**Formatting:**
- `dotnet format` (verify: `dotnet format --verify-no-changes`)
- Project-specific: `dotnet format <Project>.csproj`
- Rules in `.editorconfig`

**Structure:** sources in project dir or `src/`, tests in test project or `tests/`, `bin/` + `obj/` git-ignored.

**Solution vs project:** if `.sln` exists, `dotnet build <Sol>.sln` builds all; `dotnet sln list` to see projects.

**Framework targeting:** `<TargetFramework>` (or `<TargetFrameworks>` for multiple) in `.csproj`.
