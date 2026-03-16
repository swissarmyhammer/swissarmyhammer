---
position_column: done
position_ordinal: j5
title: Fix 9 failing doctests in llama-agent/src/chat_template.rs
---
9 doctests fail in chat_template.rs: Qwen3CoderToolParser::convert_parameter_value (line 3042), convert_by_schema_type (line 2845), convert_parameter_with_schema (line 2988), handle_empty_values (line 2784), parse_nested_parameters_with_schema (line 2939), new_with_schema (line 2658), ToolParserFactory::create_parser (line 1156), create_parser_with_schema (line 1199), create_streaming_parser (line 1260). #test-failure