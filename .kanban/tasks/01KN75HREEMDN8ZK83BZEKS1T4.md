---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9a80
title: Test templating engine builder and template partials — 56-71% coverage
---
Files:\n- swissarmyhammer-templating/src/engine.rs: 25/44 (56.8%) — with_parser, with_partials, with_plugins, plugin_registry, create_template, DummyPartialLoader all untested\n- swissarmyhammer-templating/src/partials.rs: 94/132 (71.2%) — LibraryPartialAdapter::names, PartialTag::parse, PartialRenderable::render_to, PartialLoaderAdapter::loader, HashMapPartialLoader::names untested\n- swissarmyhammer-templating/src/template.rs: 63/97 (64.9%) — with_partials, render_with_context_and_timeout, create_parser_with_partials untested\n\nNeed tests for engine builder methods, partial loading/rendering, and template timeout rendering.\n\n#coverage-gap #coverage-gap