---
title: Java (Maven) Project Guidelines
description: Best practices and tooling for Maven projects
partial: true
---

### Java (Maven) Project Guidelines

**Testing — do NOT glob; Maven discovers tests in `src/test/java/`:**
- All: `mvn test`
- Class: `mvn test -Dtest=ClassName`
- Method: `mvn test -Dtest=ClassName#methodName`
- Plus integration: `mvn verify`

**Common commands:**
- Clean build: `mvn clean install`
- Compile: `mvn compile`
- Package (JAR/WAR): `mvn package`
- Skip tests: `mvn install -DskipTests`
- Run (if configured): `mvn exec:java`
- Dep tree: `mvn dependency:tree`; unused: `mvn dependency:analyze`

**Formatting** (if configured in `pom.xml`):
- Spotless: `mvn spotless:apply` / `spotless:check`
- google-java-format plugin: check `pom.xml`

**Project structure:** `src/main/java`, `src/main/resources`, `src/test/java`, `target/` git-ignored. Config: `pom.xml`.

**Multi-module:** check parent `pom.xml` in root. Specific module: `mvn clean install -pl <module>`.
