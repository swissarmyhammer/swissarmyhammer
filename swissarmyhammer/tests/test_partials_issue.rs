use swissarmyhammer::{Prompt, PromptLibrary};
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::PromptResolver;

#[test]
fn test_partials_with_liquid_extension() {
    // Create a library and add a partial
    let mut library = PromptLibrary::new();

    // Add the partial prompt (override the builtin principals partial)
    let partial = Prompt::new(
        "principals",
        "{% partial %}\n\n## Principals\n\nDon't hold back!",
    );
    library.add(partial).unwrap();

    // Add a main prompt that uses the partial
    let main_prompt = Prompt::new(
        "do_next_issue",
        "## Goal\n\n{% render \"principals\" %}\n\nDo the work!",
    );
    library.add(main_prompt).unwrap();

    // Try to render the main prompt with partials support
    let template_context = TemplateContext::new();
    let library_arc = std::sync::Arc::new(library);
    match library_arc.render("do_next_issue", &template_context) {
        Ok(result) => {
            println!("Success:\n{result}");
            assert!(result.contains("Principals"));
            assert!(result.contains("Don't hold back"));
        }
        Err(e) => {
            println!("Error: {e}");
            panic!("Failed to render prompt with partial: {e}");
        }
    }
}

#[test]
fn test_partials_without_extension() {
    // Create a library and add a partial
    let mut library = PromptLibrary::new();

    // Add the partial prompt (without extension)
    let partial = Prompt::new(
        "principals",
        "{% partial %}\n\n## Principals\n\nDon't hold back!",
    );
    library.add(partial).unwrap();

    // Add a main prompt that uses the partial
    let main_prompt = Prompt::new(
        "do_next_issue",
        "## Goal\n\n{% render \"principals\" %}\n\nDo the work!",
    );
    library.add(main_prompt).unwrap();

    // Try to render the main prompt with partials support
    let template_context = TemplateContext::new();
    match library.render("do_next_issue", &template_context) {
        Ok(result) => {
            println!("Success:\n{result}");
            assert!(result.contains("Principals"));
            assert!(result.contains("Don't hold back"));
        }
        Err(e) => {
            println!("Error: {e}");
            panic!("Failed to render prompt with partial: {e}");
        }
    }
}
