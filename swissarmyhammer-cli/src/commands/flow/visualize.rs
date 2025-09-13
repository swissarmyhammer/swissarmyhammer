//! Generate execution visualization command implementation

use super::shared::{create_local_workflow_run_storage, parse_workflow_run_id};
use crate::cli::VisualizationFormat;
use swissarmyhammer::{Result, SwissArmyHammerError};
use swissarmyhammer_workflow::ExecutionVisualizer;

/// Execute the visualize workflow command
pub async fn execute_visualize_command(
    run_id: String,
    format: VisualizationFormat,
    output: Option<String>,
    timing: bool,
    counts: bool,
    path_only: bool,
) -> Result<()> {
    let run_id_typed = parse_workflow_run_id(&run_id)?;
    let run_storage = create_local_workflow_run_storage()?;

    let run = run_storage
        .get_run(&run_id_typed)
        .map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to load workflow run {run_id}: {e}"),
        })?;

    let visualizer = ExecutionVisualizer::new();

    let trace = visualizer.generate_trace(&run);

    let visualization_result = match format {
        VisualizationFormat::Mermaid => {
            generate_mermaid_visualization(&run, timing, counts, path_only)
        }
        VisualizationFormat::Dot => generate_dot_visualization(&run, timing, counts, path_only),
        VisualizationFormat::Json => {
            serde_json::to_string_pretty(&trace).map_err(|e| SwissArmyHammerError::Other {
                message: format!("JSON serialization error: {}", e),
            })
        }
        VisualizationFormat::Html => generate_html_visualization(&run, timing, counts, path_only),
    };

    let visualization = visualization_result.map_err(|e| SwissArmyHammerError::Other {
        message: format!("Failed to generate visualization: {e}"),
    })?;

    if let Some(output_path) = output {
        std::fs::write(&output_path, &visualization).map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to write visualization to {output_path}: {e}"),
        })?;
        println!("📊 Visualization written to: {output_path}");
    } else {
        println!("{visualization}");
    }

    Ok(())
}

/// Generate Mermaid diagram visualization
fn generate_mermaid_visualization(
    run: &swissarmyhammer_workflow::WorkflowRun,
    timing: bool,
    counts: bool,
    path_only: bool,
) -> Result<String> {
    let mut mermaid = String::from("graph TD\n");

    if path_only {
        // Show only the execution path
        mermaid.push_str(&format!("    Start --> {}\n", run.workflow.initial_state));
        for (state_id, _) in &run.history {
            mermaid.push_str(&format!(
                "    {} --> {}\n",
                run.workflow.initial_state, state_id
            ));
        }
        mermaid.push_str(&format!("    {} --> End\n", run.current_state));
    } else {
        // Show all states and transitions
        for (state_id, state) in &run.workflow.states {
            let label = if timing && !run.history.is_empty() {
                // Find timing info for this state from history
                let timing_info = run
                    .history
                    .iter()
                    .find(|(id, _)| id == state_id)
                    .map(|(_, timestamp)| timestamp.format("%H:%M:%S").to_string())
                    .unwrap_or_default();
                format!("{}[{}\\n{}]", state_id, state.description, timing_info)
            } else if counts {
                // Show transition count (simplified - just whether visited)
                let visited = run.history.iter().any(|(id, _)| id == state_id);
                let count_info = if visited { "✓" } else { "○" };
                format!("{}[{}\\n{}]", state_id, state.description, count_info)
            } else {
                format!("{}[{}]", state_id, state.description)
            };
            mermaid.push_str(&format!("    {}\n", label));
        }

        // Add transitions
        for transition in &run.workflow.transitions {
            mermaid.push_str(&format!(
                "    {} --> {}\n",
                transition.from_state, transition.to_state
            ));
        }
    }

    Ok(mermaid)
}

/// Generate Graphviz DOT visualization
fn generate_dot_visualization(
    run: &swissarmyhammer_workflow::WorkflowRun,
    timing: bool,
    counts: bool,
    path_only: bool,
) -> Result<String> {
    let mut dot = String::from("digraph workflow {\n    rankdir=TD;\n");

    if path_only {
        // Show only the execution path
        dot.push_str(&format!("    Start -> {};\n", run.workflow.initial_state));
        for (state_id, _) in &run.history {
            dot.push_str(&format!(
                "    {} -> {};\n",
                run.workflow.initial_state, state_id
            ));
        }
        dot.push_str(&format!("    {} -> End;\n", run.current_state));
    } else {
        // Show all states
        for (state_id, state) in &run.workflow.states {
            let label = if timing && !run.history.is_empty() {
                let timing_info = run
                    .history
                    .iter()
                    .find(|(id, _)| id == state_id)
                    .map(|(_, timestamp)| timestamp.format("%H:%M:%S").to_string())
                    .unwrap_or_default();
                format!("{}\\n{}\\n{}", state_id, state.description, timing_info)
            } else if counts {
                let visited = run.history.iter().any(|(id, _)| id == state_id);
                let count_info = if visited { "✓" } else { "○" };
                format!("{}\\n{}\\n{}", state_id, state.description, count_info)
            } else {
                format!("{}\\n{}", state_id, state.description)
            };

            let shape = if state.is_terminal {
                "doublecircle"
            } else {
                "box"
            };
            dot.push_str(&format!(
                "    {} [label=\"{}\" shape={}];\n",
                state_id, label, shape
            ));
        }

        // Add transitions
        for transition in &run.workflow.transitions {
            dot.push_str(&format!(
                "    {} -> {};\n",
                transition.from_state, transition.to_state
            ));
        }
    }

    dot.push_str("}\n");
    Ok(dot)
}

/// Generate HTML visualization
fn generate_html_visualization(
    run: &swissarmyhammer_workflow::WorkflowRun,
    timing: bool,
    counts: bool,
    path_only: bool,
) -> Result<String> {
    let mut html = String::from(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Workflow Visualization</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .state { 
            border: 1px solid #ccc; 
            padding: 10px; 
            margin: 5px; 
            border-radius: 5px; 
            background: #f9f9f9; 
        }
        .terminal { background: #e7f3ff; }
        .current { background: #fff2e7; font-weight: bold; }
        .visited { background: #e7ffe7; }
        .path { font-size: 18px; color: #666; }
    </style>
</head>
<body>
"#,
    );

    html.push_str(&format!("<h1>Workflow Run: {}</h1>\n", run.workflow.name));
    html.push_str(&format!(
        "<p><strong>Run ID:</strong> {}</p>\n",
        super::shared::workflow_run_id_to_string(&run.id)
    ));
    html.push_str(&format!(
        "<p><strong>Status:</strong> {:?}</p>\n",
        run.status
    ));
    html.push_str(&format!(
        "<p><strong>Current State:</strong> {}</p>\n",
        run.current_state
    ));

    if path_only {
        html.push_str("<h2>Execution Path</h2>\n<div class=\"path\">\n");
        html.push_str(&format!("Start → {} ", run.workflow.initial_state));
        for (state_id, _) in &run.history {
            html.push_str(&format!("→ {} ", state_id));
        }
        html.push_str("→ End\n</div>\n");
    } else {
        html.push_str("<h2>Workflow States</h2>\n");

        for (state_id, state) in &run.workflow.states {
            let visited = run.history.iter().any(|(id, _)| id == state_id);
            let is_current = state_id == &run.current_state;

            let mut class = "state".to_string();
            if state.is_terminal {
                class.push_str(" terminal");
            }
            if is_current {
                class.push_str(" current");
            } else if visited {
                class.push_str(" visited");
            }

            html.push_str(&format!("<div class=\"{}\">\n", class));
            html.push_str(&format!("<h3>{}</h3>\n", state_id));
            html.push_str(&format!("<p>{}</p>\n", state.description));

            if timing && visited {
                if let Some((_, timestamp)) = run.history.iter().find(|(id, _)| id == state_id) {
                    html.push_str(&format!(
                        "<p><em>Executed at: {}</em></p>\n",
                        timestamp.format("%Y-%m-%d %H:%M:%S UTC")
                    ));
                }
            }

            if counts {
                html.push_str(&format!(
                    "<p><strong>Visited:</strong> {}</p>\n",
                    if visited { "Yes" } else { "No" }
                ));
            }

            html.push_str("</div>\n");
        }

        html.push_str("<h2>Transitions</h2>\n<ul>\n");
        for transition in &run.workflow.transitions {
            html.push_str(&format!(
                "<li>{} → {}</li>\n",
                transition.from_state, transition.to_state
            ));
        }
        html.push_str("</ul>\n");
    }

    html.push_str("</body>\n</html>");
    Ok(html)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::VisualizationFormat;

    #[tokio::test]
    async fn test_execute_visualize_command_invalid_run_id() -> Result<()> {
        let result = execute_visualize_command(
            "invalid-run-id".to_string(),
            VisualizationFormat::Mermaid,
            None,
            false,
            false,
            false,
        )
        .await;

        // Should fail with invalid run ID
        assert!(
            result.is_err(),
            "Visualize command with invalid run ID should fail"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_visualize_command_json_format() -> Result<()> {
        let result = execute_visualize_command(
            "invalid-run-id".to_string(),
            VisualizationFormat::Json,
            None,
            false,
            false,
            false,
        )
        .await;

        // Should still fail with invalid run ID
        assert!(
            result.is_err(),
            "Visualize command with JSON format should fail with invalid run ID"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_visualize_command_with_output_file() -> Result<()> {
        let result = execute_visualize_command(
            "invalid-run-id".to_string(),
            VisualizationFormat::Html,
            Some("/tmp/test_output.html".to_string()),
            false,
            false,
            false,
        )
        .await;

        // Should still fail with invalid run ID
        assert!(
            result.is_err(),
            "Visualize command with output file should fail with invalid run ID"
        );
        Ok(())
    }
}
