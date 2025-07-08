class MyClass {
    // This should match: function inside class with console.log but no return
    debug() {
        console.log("debugging");
        this.process();
    }

    // This should NOT match: has return statement
    calculate() {
        console.log("calculating");
        return 42;
    }

    // This should NOT match: no console.log
    process() {
        this.data = "processed";
    }
}

// This should NOT match: not inside a class
function standalone() {
    console.log("standalone");
}
