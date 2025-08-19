//! Test-driven development for side effect detection functionality
//!
//! Tests define the expected behavior for detecting side effects in code fragments,
//! including function calls, global mutations, I/O operations, and asynchronous operations.

use ast_grep_mcp::refactoring::capture_analysis::{
    CaptureAnalysisEngine, SideEffect
};

#[cfg(test)]
mod side_effect_tests {
    use super::*;

    #[test]
    fn test_function_call_detection() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function process() {
    let data = getData();
    // Extract this
    processData(data);
    sendToServer(data);
}
"#;
        
        let fragment = "processData(data);\nsendToServer(data);";
        let analysis = engine.analyze_capture_simple(fragment, code, "javascript")
            .expect("Should analyze function calls");
        
        // Should detect function call side effects
        assert_eq!(analysis.side_effects.len(), 2);
        
        if let SideEffect::FunctionCall { name, .. } = &analysis.side_effects[0] {
            assert_eq!(name, "processData");
        } else {
            panic!("Expected function call side effect");
        }
        
        if let SideEffect::FunctionCall { name, .. } = &analysis.side_effects[1] {
            assert_eq!(name, "sendToServer");
        } else {
            panic!("Expected function call side effect");
        }
    }

    #[test]
    fn test_global_mutation_detection() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
let globalCounter = 0;
let globalState = {};

function updateGlobals() {
    // Extract this
    globalCounter += 1;
    globalState.value = "updated";
    window.localStorage.setItem("key", "value");
}
"#;
        
        let fragment = "globalCounter += 1;\nglobalState.value = \"updated\";\nwindow.localStorage.setItem(\"key\", \"value\");";
        let analysis = engine.analyze_capture_simple(fragment, code, "javascript")
            .expect("Should analyze global mutations");
        
        // Should detect global mutations and function calls
        assert!(analysis.side_effects.len() >= 2);
        
        // Check for global mutations
        let global_mutations: Vec<_> = analysis.side_effects.iter()
            .filter_map(|effect| match effect {
                SideEffect::GlobalMutation { variable } => Some(variable),
                _ => None,
            })
            .collect();
        
        assert!(global_mutations.contains(&&"globalCounter".to_string()));
    }

    #[test]
    fn test_io_operation_detection() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function performIO() {
    // Extract this
    console.log("Processing...");
    console.error("Warning!");
    console.warn("Deprecated");
    alert("Done!");
}
"#;
        
        let fragment = "console.log(\"Processing...\");\nconsole.error(\"Warning!\");\nconsole.warn(\"Deprecated\");\nalert(\"Done!\");";
        let analysis = engine.analyze_capture_simple(fragment, code, "javascript")
            .expect("Should analyze I/O operations");
        
        // Should detect I/O operations
        assert!(analysis.side_effects.len() >= 4);
        
        // Check for I/O operations
        let io_operations: Vec<_> = analysis.side_effects.iter()
            .filter_map(|effect| match effect {
                SideEffect::IOOperation { operation_type, .. } => Some(operation_type),
                _ => None,
            })
            .collect();
        
        assert!(io_operations.contains(&&"console_output".to_string()));
    }

    #[test]
    fn test_async_operation_detection() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
async function performAsync() {
    // Extract this
    await fetchData();
    setTimeout(() => console.log("delayed"), 1000);
    setInterval(updateUI, 500);
}
"#;
        
        let fragment = "await fetchData();\nsetTimeout(() => console.log(\"delayed\"), 1000);\nsetInterval(updateUI, 500);";
        let analysis = engine.analyze_capture_simple(fragment, code, "javascript")
            .expect("Should analyze async operations");
        
        // Should detect async operations
        assert!(analysis.side_effects.len() >= 3);
        
        // Check for async operations
        let async_ops: Vec<_> = analysis.side_effects.iter()
            .filter_map(|effect| match effect {
                SideEffect::AsyncOperation { operation_type, .. } => Some(operation_type),
                _ => None,
            })
            .collect();
        
        assert!(async_ops.contains(&&"timer".to_string()));
    }

    #[test]
    fn test_dom_manipulation_detection() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function updateDOM() {
    let element = document.getElementById("myDiv");
    // Extract this
    element.innerHTML = "New content";
    element.style.color = "red";
    document.body.appendChild(element);
    element.addEventListener("click", handler);
}
"#;
        
        let fragment = "element.innerHTML = \"New content\";\nelement.style.color = \"red\";\ndocument.body.appendChild(element);\nelement.addEventListener(\"click\", handler);";
        let analysis = engine.analyze_capture_simple(fragment, code, "javascript")
            .expect("Should analyze DOM manipulation");
        
        // Should detect DOM manipulations
        assert!(analysis.side_effects.len() >= 3);
        
        // Check for DOM manipulations
        let dom_effects: Vec<_> = analysis.side_effects.iter()
            .filter_map(|effect| match effect {
                SideEffect::DOMManipulation { element, action } => Some((element, action)),
                _ => None,
            })
            .collect();
        
        assert!(!dom_effects.is_empty());
    }

    #[test]
    fn test_network_operation_detection() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function networkOperations() {
    // Extract this
    fetch("/api/data");
    XMLHttpRequest().open("GET", "/api/users");
    navigator.sendBeacon("/analytics", data);
}
"#;
        
        let fragment = "fetch(\"/api/data\");\nXMLHttpRequest().open(\"GET\", \"/api/users\");\nnavigator.sendBeacon(\"/analytics\", data);";
        let analysis = engine.analyze_capture_simple(fragment, code, "javascript")
            .expect("Should analyze network operations");
        
        // Should detect network operations
        assert!(analysis.side_effects.len() >= 3);
        
        // Check for network operations
        let network_ops: Vec<_> = analysis.side_effects.iter()
            .filter_map(|effect| match effect {
                SideEffect::NetworkOperation { url, method } => Some((url, method)),
                _ => None,
            })
            .collect();
        
        assert!(!network_ops.is_empty());
    }

    #[test]
    fn test_pure_function_no_side_effects() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function pureCalculation() {
    let x = 5;
    let y = 10;
    // Extract this - pure calculation
    let sum = x + y;
    let product = x * y;
    let average = sum / 2;
    return average;
}
"#;
        
        let fragment = "let sum = x + y;\nlet product = x * y;\nlet average = sum / 2;\nreturn average;";
        let analysis = engine.analyze_capture_simple(fragment, code, "javascript")
            .expect("Should analyze pure function");
        
        // Should detect no side effects
        assert_eq!(analysis.side_effects.len(), 0);
    }
}