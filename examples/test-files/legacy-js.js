// Legacy JavaScript code with patterns to detect

function processData(data) {
    // Use of var instead of let
    var result = [];
    var index = 0;

    // Console.log usage (should be replaced with logger)
    console.log("Processing data:", data);

    // Defensive method calls (can use optional chaining)
    if (data.validate && data.validate()) {
        console.log("Data is valid");
    }

    // Another defensive pattern
    data.transform && data.transform();

    // Nested ternary (hard to read)
    var status = data.ready ? (data.valid ? "good" : "invalid") : "pending";

    console.log("Status:", status);

    return result;
}

class DataProcessor {
    constructor() {
        var initialized = true;
        console.log("DataProcessor initialized");
    }

    // Method with console.log but no return (matches complex rule)
    debug() {
        console.log("Debug info");
        this.checkState();
    }

    // Method with console.log AND return (doesn't match complex rule)
    calculate() {
        console.log("Calculating");
        return 42;
    }

    // Method without console.log (doesn't match complex rule)
    process() {
        this.data = "processed";
    }
}
