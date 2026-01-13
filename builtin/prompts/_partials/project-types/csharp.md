---
title: C# / .NET Project Guidelines
description: Best practices and tooling for .NET projects
partial: true
---

### C# / .NET Project Guidelines

**Common Commands:**
- Restore dependencies: `dotnet restore`
- Build: `dotnet build`
- **Run ALL tests:** `dotnet test` (discovers and runs all xUnit/NUnit/MSTest tests automatically)
- **Run tests with verbosity:** `dotnet test --verbosity normal`
- **Run specific test:** `dotnet test --filter FullyQualifiedName~TestMethodName`
- **Run tests with coverage:** `dotnet test --collect:"XPlat Code Coverage"`
- Run: `dotnet run`
- Clean: `dotnet clean`
- Publish: `dotnet publish -c Release`
- Add package: `dotnet add package PackageName`

**IMPORTANT:** Do NOT glob for test files. .NET automatically discovers test projects and tests. Use `dotnet test` to run all tests.

**Best Practices:**
- Run `dotnet restore` after cloning or when dependencies change
- Use `dotnet build` for development, `dotnet publish` for deployment
- Specify configuration: `-c Debug` or `-c Release`
- Run tests with coverage: `dotnet test --collect:"XPlat Code Coverage"`

**Project Structure:**
- Source code: Project directory or `src/`
- Tests: Test project directory or `tests/`
- Solution file: `*.sln` (groups multiple projects)
- Project file: `*.csproj` or `*.fsproj`
- Build output: `bin/` and `obj/` (git-ignored)

**Solution vs Project:**
- If `.sln` exists, build entire solution: `dotnet build SolutionName.sln`
- Build specific project: `dotnet build ProjectName.csproj`
- List projects: Check `dotnet sln list`

**Framework Targeting:**
- Check `<TargetFramework>` in `.csproj` for required .NET version
- Multiple targets: `<TargetFrameworks>` (plural)
