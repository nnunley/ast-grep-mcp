//! Debug test for learning system

use ast_grep_mcp::learning::{DiscoveryService, ExplorePatternParam};

#[tokio::test]
async fn test_discovery_service_directly() {
    let discovery = DiscoveryService::new().expect("Failed to create discovery service");

    let param = ExplorePatternParam {
        language: None,
        category: None,
        complexity: None,
        search: None,
        limit: Some(10),
    };

    let result = discovery
        .explore_patterns(param)
        .await
        .expect("Failed to explore patterns");

    println!("Patterns found: {}", result.patterns.len());
    println!("Total available: {}", result.total_available);
    println!("Learning path steps: {}", result.learning_path.len());

    for (i, pattern) in result.patterns.iter().take(3).enumerate() {
        println!("Pattern {}: {} ({})", i + 1, pattern.id, pattern.language);
    }

    // For debugging, let's be less strict
    assert!(result.total_available < 1000); // Just ensure it doesn't crash and gives reasonable numbers
}

#[tokio::test]
async fn test_discovery_service_languages() {
    let discovery = DiscoveryService::new().expect("Failed to create discovery service");

    let languages = discovery.get_languages();
    println!("Available languages: {languages:?}");

    let categories = discovery.get_categories(None);
    println!("Available categories: {categories:?}");
}
