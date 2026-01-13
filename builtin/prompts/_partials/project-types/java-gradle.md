---
title: Java (Gradle) Project Guidelines
description: Best practices and tooling for Gradle projects
partial: true
---

### Java (Gradle) Project Guidelines

**Gradle Wrapper:**
- Use wrapper for consistent Gradle version: `./gradlew` (Unix) or `gradlew.bat` (Windows)
- Wrapper is preferred over system Gradle installation

**Common Commands:**
- Build: `./gradlew build`
- Clean build: `./gradlew clean build`
- **Run ALL tests:** `./gradlew test` (discovers and runs all tests automatically)
- **Run tests with output:** `./gradlew test --info`
- **Run specific test class:** `./gradlew test --tests ClassName`
- **Run specific test method:** `./gradlew test --tests ClassName.methodName`
- **Run tests and checks:** `./gradlew check` (includes tests + linting)
- Run: `./gradlew run` (if application plugin is applied)
- List tasks: `./gradlew tasks`
- Dependencies: `./gradlew dependencies`
- Skip tests: `./gradlew build -x test`

**IMPORTANT:** Do NOT glob for test files. Gradle automatically discovers tests in `src/test/`. Use `./gradlew test` to run all tests.

**Best Practices:**
- Always use the Gradle wrapper (`./gradlew`) for consistency
- Run `./gradlew clean` before full builds to avoid stale artifacts
- Use `./gradlew check` for validation including tests and linting
- Check for `gradle.properties` for project-specific settings

**Project Structure:**
- Source code: `src/main/java/` or `src/main/kotlin/`
- Resources: `src/main/resources/`
- Tests: `src/test/java/` or `src/test/kotlin/`
- Test resources: `src/test/resources/`
- Build output: `build/` (git-ignored)
- Configuration: `build.gradle` or `build.gradle.kts`

**Multi-Project Builds:**
- Check for `settings.gradle` defining subprojects
- Build from root: `./gradlew build`
- Build specific project: `./gradlew :project-name:build`
