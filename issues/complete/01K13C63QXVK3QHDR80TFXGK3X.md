I get a failure when I search

 cargo run search query "duckdb"
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.17s
     Running `target/debug/swissarmyhammer search query duckdb`
2025-07-26T12:42:38.384558Z  INFO swissarmyhammer: Running search command
🔍 Starting semantic search query...
Searching for: duckdb
Result limit: 10
2025-07-26T12:42:38.412018Z  INFO swissarmyhammer::semantic::storage: Initializing DuckDB vector storage at: /Users/wballard/.swissarmyhammer/semantic.db
2025-07-26T12:42:38.416349Z  INFO swissarmyhammer::semantic::storage: Database schema initialized successfully
2025-07-26T12:42:38.416409Z  INFO swissarmyhammer::semantic::embedding: Initializing fastembed embedding engine with model: all-MiniLM-L6-v2
2025-07-26T12:42:38.424429Z  INFO ort::logging: Session Options {  execution_mode:0 execution_order:DEFAULT enable_profiling:0 optimized_model_filepath:"" enable_mem_pattern:1 enable_mem_reuse:1 enable_cpu_mem_arena:1 profile_file_prefix:onnxruntime_profile_ session_logid: session_log_severity_level:-1 session_log_verbosity_level:0 max_num_graph_transformation_steps:10 graph_optimization_level:3 intra_op_param:OrtThreadPoolParams { thread_pool_size: 16 auto_set_affinity: 0 allow_spinning: 1 dynamic_block_base_: 0 stack_size: 0 affinity_str:  set_denormal_as_zero: 0 } inter_op_param:OrtThreadPoolParams { thread_pool_size: 0 auto_set_affinity: 0 allow_spinning: 1 dynamic_block_base_: 0 stack_size: 0 affinity_str:  set_denormal_as_zero: 0 } use_per_session_threads:1 thread_pool_allow_spinning:1 use_deterministic_compute:0 ep_selection_policy:0 config_options: {  } }
2025-07-26T12:42:38.424443Z  INFO ort::logging: Flush-to-zero and denormal-as-zero are off
2025-07-26T12:42:38.424453Z  INFO ort::logging: Creating and using per session threadpools since use_per_session_threads_ is true
2025-07-26T12:42:38.424458Z  INFO ort::logging: Dynamic block base set to 0
2025-07-26T12:42:38.437690Z  INFO ort::logging: Initializing session.
2025-07-26T12:42:38.437706Z  INFO ort::logging: Adding default CPU execution provider.
2025-07-26T12:42:38.437781Z  INFO ort::logging: Creating BFCArena for Cpu with following configs: initial_chunk_size_bytes: 1048576 max_dead_bytes_per_chunk: 134217728 initial_growth_chunk_size_bytes: 2097152 max_power_of_two_extend_bytes: 1073741824 memory limit: 18446744073709551615 arena_extend_strategy: 0
2025-07-26T12:42:38.439933Z  INFO ort::logging: This model does not have any local functions defined. AOT Inlining is not performed
2025-07-26T12:42:38.440016Z  INFO ort::logging: GraphTransformer EnsureUniqueDQForNodeUnit modified: 0 with status: OK
2025-07-26T12:42:38.440116Z  INFO ort::logging: GraphTransformer Level1_RuleBasedTransformer modified: 0 with status: OK
2025-07-26T12:42:38.440147Z  INFO ort::logging: GraphTransformer DoubleQDQPairsRemover modified: 0 with status: OK
2025-07-26T12:42:38.440374Z  INFO ort::logging: Total shared scalar initializer count: 132
2025-07-26T12:42:38.440383Z  INFO ort::logging: GraphTransformer ConstantSharing modified: 1 with status: OK
2025-07-26T12:42:38.441407Z  INFO ort::logging: GraphTransformer CommonSubexpressionElimination modified: 1 with status: OK
2025-07-26T12:42:38.442253Z  INFO ort::logging: GraphTransformer ConstantFolding modified: 0 with status: OK
2025-07-26T12:42:38.442300Z  INFO ort::logging: GraphTransformer MatMulAddFusion modified: 0 with status: OK
2025-07-26T12:42:38.442339Z  INFO ort::logging: Fused reshape node: /encoder/layer.0/attention/self/Reshape_output_0
2025-07-26T12:42:38.442348Z  INFO ort::logging: Fused reshape node: /encoder/layer.0/attention/self/Reshape_2_output_0
2025-07-26T12:42:38.442355Z  INFO ort::logging: Fused reshape node: /encoder/layer.0/attention/self/Reshape_1_output_0
2025-07-26T12:42:38.442362Z  INFO ort::logging: Fused reshape node: /encoder/layer.0/attention/self/Reshape_3_output_0
2025-07-26T12:42:38.442370Z  INFO ort::logging: Fused reshape node: /encoder/layer.1/attention/self/Reshape_output_0
2025-07-26T12:42:38.442377Z  INFO ort::logging: Fused reshape node: /encoder/layer.1/attention/self/Reshape_2_output_0
2025-07-26T12:42:38.442382Z  INFO ort::logging: Fused reshape node: /encoder/layer.1/attention/self/Reshape_1_output_0
2025-07-26T12:42:38.442389Z  INFO ort::logging: Fused reshape node: /encoder/layer.1/attention/self/Reshape_3_output_0
2025-07-26T12:42:38.442395Z  INFO ort::logging: Fused reshape node: /encoder/layer.2/attention/self/Reshape_output_0
2025-07-26T12:42:38.442400Z  INFO ort::logging: Fused reshape node: /encoder/layer.2/attention/self/Reshape_2_output_0
2025-07-26T12:42:38.442406Z  INFO ort::logging: Fused reshape node: /encoder/layer.2/attention/self/Reshape_1_output_0
2025-07-26T12:42:38.442447Z  INFO ort::logging: Fused reshape node: /encoder/layer.2/attention/self/Reshape_3_output_0
2025-07-26T12:42:38.442464Z  INFO ort::logging: Fused reshape node: /encoder/layer.3/attention/self/Reshape_output_0
2025-07-26T12:42:38.442474Z  INFO ort::logging: Fused reshape node: /encoder/layer.3/attention/self/Reshape_2_output_0
2025-07-26T12:42:38.442482Z  INFO ort::logging: Fused reshape node: /encoder/layer.3/attention/self/Reshape_1_output_0
2025-07-26T12:42:38.442491Z  INFO ort::logging: Fused reshape node: /encoder/layer.3/attention/self/Reshape_3_output_0
2025-07-26T12:42:38.442499Z  INFO ort::logging: Fused reshape node: /encoder/layer.4/attention/self/Reshape_output_0
2025-07-26T12:42:38.442506Z  INFO ort::logging: Fused reshape node: /encoder/layer.4/attention/self/Reshape_2_output_0
2025-07-26T12:42:38.442512Z  INFO ort::logging: Fused reshape node: /encoder/layer.4/attention/self/Reshape_1_output_0
2025-07-26T12:42:38.442519Z  INFO ort::logging: Fused reshape node: /encoder/layer.4/attention/self/Reshape_3_output_0
2025-07-26T12:42:38.442527Z  INFO ort::logging: Fused reshape node: /encoder/layer.5/attention/self/Reshape_output_0
2025-07-26T12:42:38.442534Z  INFO ort::logging: Fused reshape node: /encoder/layer.5/attention/self/Reshape_2_output_0
2025-07-26T12:42:38.442539Z  INFO ort::logging: Fused reshape node: /encoder/layer.5/attention/self/Reshape_1_output_0
2025-07-26T12:42:38.442545Z  INFO ort::logging: Fused reshape node: /encoder/layer.5/attention/self/Reshape_3_output_0
2025-07-26T12:42:38.442548Z  INFO ort::logging: Total fused reshape node count: 24
2025-07-26T12:42:38.442551Z  INFO ort::logging: GraphTransformer ReshapeFusion modified: 1 with status: OK
2025-07-26T12:42:38.443251Z  INFO ort::logging: Removing initializer '/encoder/layer.5/attention/self/Constant_15_output_0'. It is no longer used by any node.
2025-07-26T12:42:38.443256Z  INFO ort::logging: Removing initializer '/encoder/layer.5/attention/self/Constant_11_output_0'. It is no longer used by any node.
2025-07-26T12:42:38.443259Z  INFO ort::logging: Removing initializer '/encoder/layer.5/attention/self/Constant_10_output_0'. It is no longer used by any node.
2025-07-26T12:42:38.443262Z  INFO ort::logging: Removing initializer '/encoder/layer.5/attention/self/Constant_8_output_0'. It is no longer used by any node.
2025-07-26T12:42:38.443304Z  INFO ort::logging: GraphTransformer FreeDimensionOverrideTransformer modified: 0 with status: OK
2025-07-26T12:42:38.443308Z  INFO ort::logging: GraphTransformer GeluFusionL1 modified: 0 with status: OK
2025-07-26T12:42:38.443311Z  INFO ort::logging: GraphTransformer LayerNormFusionL1 modified: 0 with status: OK
2025-07-26T12:42:38.443335Z  INFO ort::logging: GraphTransformer QDQPropagationTransformer modified: 0 with status: OK
2025-07-26T12:42:38.443354Z  INFO ort::logging: GraphTransformer WeightBiasQuantization modified: 0 with status: OK
2025-07-26T12:42:38.443372Z  INFO ort::logging: GraphTransformer EnsureUniqueDQForNodeUnit modified: 0 with status: OK
2025-07-26T12:42:38.443389Z  INFO ort::logging: GraphTransformer RocmBlasAltImpl modified: 0 with status: OK
2025-07-26T12:42:38.443568Z  INFO ort::logging: GraphTransformer TransposeOptimizer modified: 0 with status: OK
2025-07-26T12:42:38.443620Z  INFO ort::logging: GraphTransformer Level1_RuleBasedTransformer modified: 0 with status: OK
2025-07-26T12:42:38.443637Z  INFO ort::logging: GraphTransformer DoubleQDQPairsRemover modified: 0 with status: OK
2025-07-26T12:42:38.443737Z  INFO ort::logging: GraphTransformer CommonSubexpressionElimination modified: 0 with status: OK
2025-07-26T12:42:38.443767Z  INFO ort::logging: GraphTransformer ConstantFolding modified: 0 with status: OK
2025-07-26T12:42:38.443791Z  INFO ort::logging: GraphTransformer MatMulAddFusion modified: 0 with status: OK
2025-07-26T12:42:38.443810Z  INFO ort::logging: GraphTransformer ReshapeFusion modified: 0 with status: OK
2025-07-26T12:42:38.443813Z  INFO ort::logging: GraphTransformer FreeDimensionOverrideTransformer modified: 0 with status: OK
2025-07-26T12:42:38.443815Z  INFO ort::logging: GraphTransformer GeluFusionL1 modified: 0 with status: OK
2025-07-26T12:42:38.443818Z  INFO ort::logging: GraphTransformer LayerNormFusionL1 modified: 0 with status: OK
2025-07-26T12:42:38.443837Z  INFO ort::logging: GraphTransformer QDQPropagationTransformer modified: 0 with status: OK
2025-07-26T12:42:38.443854Z  INFO ort::logging: GraphTransformer WeightBiasQuantization modified: 0 with status: OK
2025-07-26T12:42:38.443871Z  INFO ort::logging: GraphTransformer EnsureUniqueDQForNodeUnit modified: 0 with status: OK
2025-07-26T12:42:38.443887Z  INFO ort::logging: GraphTransformer RocmBlasAltImpl modified: 0 with status: OK
2025-07-26T12:42:38.444118Z  INFO ort::logging: GraphTransformer Level2_RuleBasedTransformer modified: 0 with status: OK
2025-07-26T12:42:38.444259Z  INFO ort::logging: GraphTransformer TransposeOptimizer_CPUExecutionProvider modified: 0 with status: OK
2025-07-26T12:42:38.444324Z  INFO ort::logging: GraphTransformer QDQSelectorActionTransformer modified: 0 with status: OK
2025-07-26T12:42:38.444342Z  INFO ort::logging: GraphTransformer GemmActivationFusion modified: 0 with status: OK
2025-07-26T12:42:38.444362Z  INFO ort::logging: GraphTransformer MatMulIntegerToFloatFusion modified: 0 with status: OK
2025-07-26T12:42:38.444380Z  INFO ort::logging: GraphTransformer DynamicQuantizeMatMulFusion modified: 0 with status: OK
2025-07-26T12:42:38.444405Z  INFO ort::logging: GraphTransformer ConvActivationFusion modified: 0 with status: OK
2025-07-26T12:42:38.444447Z  INFO ort::logging: GraphTransformer GeluFusionL2 modified: 1 with status: OK
2025-07-26T12:42:38.445097Z  INFO ort::logging: Removing initializer '/encoder/layer.5/intermediate/intermediate_act_fn/Constant_2_output_0'. It is no longer used by any node.
2025-07-26T12:42:38.445101Z  INFO ort::logging: Removing initializer '/encoder/layer.5/intermediate/intermediate_act_fn/Constant_output_0'. It is no longer used by any node.
2025-07-26T12:42:38.445210Z  INFO ort::logging: GraphTransformer LayerNormFusionL2 modified: 1 with status: OK
2025-07-26T12:42:38.445700Z  INFO ort::logging: Removing initializer '/encoder/layer.5/output/LayerNorm/Constant_1_output_0'. It is no longer used by any node.
2025-07-26T12:42:38.445704Z  INFO ort::logging: Removing initializer '/encoder/layer.5/output/LayerNorm/Constant_output_0'. It is no longer used by any node.
2025-07-26T12:42:38.445742Z  INFO ort::logging: GraphTransformer SimplifiedLayerNormFusion modified: 0 with status: OK
2025-07-26T12:42:38.445777Z  INFO ort::logging: GraphTransformer AttentionFusion modified: 0 with status: OK
2025-07-26T12:42:38.445799Z  INFO ort::logging: GraphTransformer EmbedLayerNormFusion modified: 0 with status: OK
2025-07-26T12:42:38.445802Z  INFO ort::logging: GraphTransformer GatherSliceToSplitFusion modified: 0 with status: OK
2025-07-26T12:42:38.445814Z  INFO ort::logging: GraphTransformer GatherToSliceFusion modified: 0 with status: OK
2025-07-26T12:42:38.445841Z  INFO ort::logging: GraphTransformer MatmulTransposeFusion modified: 0 with status: OK
2025-07-26T12:42:38.445868Z  INFO ort::logging: GraphTransformer BiasGeluFusion modified: 1 with status: OK
2025-07-26T12:42:38.446322Z  INFO ort::logging: GraphTransformer GroupQueryAttentionFusion modified: 0 with status: OK
2025-07-26T12:42:38.446342Z  INFO ort::logging: GraphTransformer SkipLayerNormFusion modified: 0 with status: OK
2025-07-26T12:42:38.446358Z  INFO ort::logging: GraphTransformer FastGeluFusion modified: 0 with status: OK
2025-07-26T12:42:38.446369Z  INFO ort::logging: GraphTransformer QuickGeluFusion modified: 0 with status: OK
2025-07-26T12:42:38.446383Z  INFO ort::logging: GraphTransformer BiasSoftmaxFusion modified: 0 with status: OK
2025-07-26T12:42:38.446395Z  INFO ort::logging: GraphTransformer BiasDropoutFusion modified: 0 with status: OK
2025-07-26T12:42:38.446435Z  INFO ort::logging: GraphTransformer MatMulScaleFusion modified: 1 with status: OK
2025-07-26T12:42:38.446873Z  INFO ort::logging: Removing initializer '/encoder/layer.5/attention/self/Constant_12_output_0'. It is no longer used by any node.
2025-07-26T12:42:38.446905Z  INFO ort::logging: GraphTransformer MatMulActivationFusion modified: 0 with status: OK
2025-07-26T12:42:38.446922Z  INFO ort::logging: GraphTransformer MatMulNBitsFusion modified: 0 with status: OK
2025-07-26T12:42:38.446934Z  INFO ort::logging: GraphTransformer QDQFinalCleanupTransformer modified: 0 with status: OK
2025-07-26T12:42:38.446946Z  INFO ort::logging: GraphTransformer Level2_RuleBasedTransformer modified: 0 with status: OK
2025-07-26T12:42:38.446979Z  INFO ort::logging: GraphTransformer QDQSelectorActionTransformer modified: 0 with status: OK
2025-07-26T12:42:38.446990Z  INFO ort::logging: GraphTransformer GemmActivationFusion modified: 0 with status: OK
2025-07-26T12:42:38.447001Z  INFO ort::logging: GraphTransformer MatMulIntegerToFloatFusion modified: 0 with status: OK
2025-07-26T12:42:38.447011Z  INFO ort::logging: GraphTransformer DynamicQuantizeMatMulFusion modified: 0 with status: OK
2025-07-26T12:42:38.447027Z  INFO ort::logging: GraphTransformer ConvActivationFusion modified: 0 with status: OK
2025-07-26T12:42:38.447039Z  INFO ort::logging: GraphTransformer GeluFusionL2 modified: 0 with status: OK
2025-07-26T12:42:38.447049Z  INFO ort::logging: GraphTransformer LayerNormFusionL2 modified: 0 with status: OK
2025-07-26T12:42:38.447060Z  INFO ort::logging: GraphTransformer SimplifiedLayerNormFusion modified: 0 with status: OK
2025-07-26T12:42:38.447074Z  INFO ort::logging: GraphTransformer AttentionFusion modified: 0 with status: OK
2025-07-26T12:42:38.447086Z  INFO ort::logging: GraphTransformer EmbedLayerNormFusion modified: 0 with status: OK
2025-07-26T12:42:38.447089Z  INFO ort::logging: GraphTransformer GatherSliceToSplitFusion modified: 0 with status: OK
2025-07-26T12:42:38.447099Z  INFO ort::logging: GraphTransformer GatherToSliceFusion modified: 0 with status: OK
2025-07-26T12:42:38.447115Z  INFO ort::logging: GraphTransformer MatmulTransposeFusion modified: 0 with status: OK
2025-07-26T12:42:38.447128Z  INFO ort::logging: GraphTransformer BiasGeluFusion modified: 0 with status: OK
2025-07-26T12:42:38.447139Z  INFO ort::logging: GraphTransformer GroupQueryAttentionFusion modified: 0 with status: OK
2025-07-26T12:42:38.447152Z  INFO ort::logging: GraphTransformer SkipLayerNormFusion modified: 0 with status: OK
2025-07-26T12:42:38.447164Z  INFO ort::logging: GraphTransformer FastGeluFusion modified: 0 with status: OK
2025-07-26T12:42:38.447176Z  INFO ort::logging: GraphTransformer QuickGeluFusion modified: 0 with status: OK
2025-07-26T12:42:38.447189Z  INFO ort::logging: GraphTransformer BiasSoftmaxFusion modified: 0 with status: OK
2025-07-26T12:42:38.447200Z  INFO ort::logging: GraphTransformer BiasDropoutFusion modified: 0 with status: OK
2025-07-26T12:42:38.447215Z  INFO ort::logging: GraphTransformer MatMulScaleFusion modified: 0 with status: OK
2025-07-26T12:42:38.447226Z  INFO ort::logging: GraphTransformer MatMulActivationFusion modified: 0 with status: OK
2025-07-26T12:42:38.447240Z  INFO ort::logging: GraphTransformer MatMulNBitsFusion modified: 0 with status: OK
2025-07-26T12:42:38.447252Z  INFO ort::logging: GraphTransformer QDQFinalCleanupTransformer modified: 0 with status: OK
2025-07-26T12:42:38.447294Z  INFO ort::logging: GraphTransformer NhwcTransformer modified: 0 with status: OK
2025-07-26T12:42:38.447310Z  INFO ort::logging: GraphTransformer ConvAddActivationFusion modified: 0 with status: OK
2025-07-26T12:42:38.447372Z  INFO ort::logging: GraphTransformer RemoveDuplicateCastTransformer modified: 0 with status: OK
2025-07-26T12:42:38.447375Z  INFO ort::logging: GraphTransformer CastFloat16Transformer modified: 0 with status: OK
2025-07-26T12:42:38.447378Z  INFO ort::logging: GraphTransformer MemcpyTransformer modified: 0 with status: OK
2025-07-26T12:42:38.447639Z  INFO ort::logging: Use DeviceBasedPartition as default
2025-07-26T12:42:38.449545Z  INFO ort::logging: Saving initialized tensors.
2025-07-26T12:42:38.449569Z  INFO ort::logging: Extending BFCArena for Cpu. bin_num:11 (requested) num_bytes: 589824 (actual) rounded_bytes:589824
2025-07-26T12:42:38.449574Z  INFO ort::logging: Extended allocation by 1048576 bytes.
2025-07-26T12:42:38.449577Z  INFO ort::logging: Total allocated bytes: 1048576
2025-07-26T12:42:38.449580Z  INFO ort::logging: Allocated memory at 0x128028000 to 0x128128000
2025-07-26T12:42:38.449634Z  INFO ort::logging: Extending BFCArena for Cpu. bin_num:0 (requested) num_bytes: 8 (actual) rounded_bytes:256
2025-07-26T12:42:38.449637Z  INFO ort::logging: Extended allocation by 2097152 bytes.
2025-07-26T12:42:38.449640Z  INFO ort::logging: Total allocated bytes: 3145728
2025-07-26T12:42:38.449642Z  INFO ort::logging: Allocated memory at 0x128130000 to 0x128330000
2025-07-26T12:42:38.449656Z  INFO ort::logging: Extending BFCArena for Cpu. bin_num:17 (requested) num_bytes: 46881792 (actual) rounded_bytes:46881792
2025-07-26T12:42:38.449661Z  INFO ort::logging: Extended allocation by 67108864 bytes.
2025-07-26T12:42:38.449663Z  INFO ort::logging: Total allocated bytes: 70254592
2025-07-26T12:42:38.449665Z  INFO ort::logging: Allocated memory at 0x1234b8000 to 0x1274b8000
2025-07-26T12:42:38.452689Z  INFO ort::logging: Extending BFCArena for Cpu. bin_num:13 (requested) num_bytes: 2359296 (actual) rounded_bytes:2359296
2025-07-26T12:42:38.452696Z  INFO ort::logging: Extended allocation by 67108864 bytes.
2025-07-26T12:42:38.452698Z  INFO ort::logging: Total allocated bytes: 137363456
2025-07-26T12:42:38.452701Z  INFO ort::logging: Allocated memory at 0x130000000 to 0x134000000
2025-07-26T12:42:38.456446Z  INFO ort::logging: Done saving initialized tensors
2025-07-26T12:42:38.456677Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.456913Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.457116Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.457318Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.457507Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 2359296
2025-07-26T12:42:38.458251Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 2359296
2025-07-26T12:42:38.459029Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.459226Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.459424Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.459611Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.459813Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 2359296
2025-07-26T12:42:38.460575Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 2359296
2025-07-26T12:42:38.461349Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.461538Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.461733Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.461936Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.462125Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 2359296
2025-07-26T12:42:38.462889Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 2359296
2025-07-26T12:42:38.463652Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.463856Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.464045Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.464247Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.464449Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 2359296
2025-07-26T12:42:38.465221Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 2359296
2025-07-26T12:42:38.465977Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.466173Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.466378Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.466583Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.466782Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 2359296
2025-07-26T12:42:38.467586Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 2359296
2025-07-26T12:42:38.468410Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.468607Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.468808Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.469006Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 589824
2025-07-26T12:42:38.469211Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 2359296
2025-07-26T12:42:38.470042Z  INFO ort::logging: Reserving memory in BFCArena for Cpu size: 2359296
2025-07-26T12:42:38.470940Z  INFO ort::logging: Session successfully initialized.
2025-07-26T12:42:38.526591Z  INFO swissarmyhammer::semantic::embedding: Successfully initialized fastembed embedding engine with 384 dimensions
❌ Search failed: Vector storage operation failed: similarity search

## Proposed Solution

Based on the error logs, the search functionality fails during the similarity search operation despite successful initialization of:
- DuckDB vector storage 
- Database schema
- FastEmbed embedding engine (384 dimensions)

My implementation plan:

1. **Investigate the Search Failure**: Examine the semantic search module to understand where the similarity search is failing
2. **Examine Vector Storage**: Look at the vector storage implementation, particularly the DuckDB integration
3. **Find Root Cause**: Identify the specific code path that's causing the "similarity search" failure
4. **Write Failing Test**: Create a test that reproduces this search issue using TDD
5. **Fix Similarity Search**: Implement the fix for the similarity search functionality
6. **Verify Fix**: Confirm the fix works by running the search command

The issue appears to be in the vector storage similarity search operation after successful initialization, suggesting a runtime error rather than a configuration problem.
