---
title: Java (Maven) Project Guidelines
description: Best practices and tooling for Maven projects
partial: true
---

### Java (Maven) Project Guidelines

**Common Commands:**
- Clean build: `mvn clean install`
- Compile: `mvn compile`
- **Run ALL tests:** `mvn test` (runs all JUnit/TestNG tests automatically)
- **Run specific test class:** `mvn test -Dtest=ClassName`
- **Run specific test method:** `mvn test -Dtest=ClassName#methodName`
- **Run integration tests:** `mvn verify` (includes `mvn test` plus integration tests)
- Package: `mvn package` (creates JAR/WAR)
- Run: `mvn exec:java` (if configured)
- Skip tests: `mvn install -DskipTests`
- Dependency tree: `mvn dependency:tree`

**IMPORTANT:** Do NOT glob for test files. Maven automatically discovers tests in `src/test/java/`. Use `mvn test` to run all tests.

**Best Practices:**
- Use `mvn clean` before full builds to avoid stale artifacts
- Run `mvn verify` for full validation including integration tests
- Check `mvn -version` to verify Java and Maven versions
- Use `mvn dependency:analyze` to find unused dependencies

**Project Structure:**
- Source code: `src/main/java/`
- Resources: `src/main/resources/`
- Tests: `src/test/java/`
- Test resources: `src/test/resources/`
- Build output: `target/` (git-ignored)
- Configuration: `pom.xml` (Maven configuration)

**Multi-Module Projects:**
- Check for parent `pom.xml` in root
- Build from root: `mvn clean install`
- Build specific module: `mvn clean install -pl module-name`
