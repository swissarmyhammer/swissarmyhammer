---
title: Java (Gradle) Project Guidelines
description: Best practices and tooling for Gradle projects
partial: true
---

### Java (Gradle) Project Guidelines

**Always use the Gradle wrapper:** `./gradlew` (Unix) / `gradlew.bat` (Windows). Never the system Gradle.

**Testing — do NOT glob; Gradle discovers tests in `src/test/`:**
- All: `./gradlew test`
- With output: `./gradlew test --info`
- Class: `./gradlew test --tests ClassName`
- Method: `./gradlew test --tests ClassName.methodName`
- Test + lint: `./gradlew check`

**Common commands:**
- Build: `./gradlew build` (clean: `./gradlew clean build`)
- Skip tests: `./gradlew build -x test`
- Run app: `./gradlew run` (if `application` plugin)
- List tasks: `./gradlew tasks`
- Deps: `./gradlew dependencies`

**Formatting:** if Spotless is configured, `./gradlew spotlessApply` / `spotlessCheck`. Discover formatter tasks: `./gradlew tasks --group formatting`.

**Project structure:** `src/main/java` (or `kotlin`), `src/main/resources`, `src/test/java`, `build/` git-ignored. Config: `build.gradle[.kts]`.

**Multi-project:** check `settings.gradle`. Specific: `./gradlew :project-name:build`.
