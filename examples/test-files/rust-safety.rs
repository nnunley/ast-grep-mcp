// Rust code with safety issues

use std::collections::HashMap;

fn main() {
    let data = vec![1, 2, 3, 4, 5];

    // Bad: using unwrap() - could panic
    let first = data.get(0).unwrap();
    println!("First element: {}", first);

    // Bad: using expect() - still could panic
    let last = data.get(10).expect("Should have last element");

    // Good: proper error handling
    if let Some(element) = data.get(0) {
        println!("First element: {}", element);
    }

    process_map();
}

fn process_map() {
    let mut map = HashMap::new();
    map.insert("key", "value");

    // Bad: unwrap on HashMap get
    let value = map.get("key").unwrap();
    println!("Value: {}", value);

    // Bad: unwrap on parsing
    let number: i32 = "42".parse().unwrap();
    println!("Number: {}", number);

    // Good: proper error handling
    match "42".parse::<i32>() {
        Ok(n) => println!("Number: {}", n),
        Err(e) => println!("Parse error: {}", e),
    }
}

struct Config {
    name: String,
    version: String,
}

impl Config {
    fn load() -> Result<Self, String> {
        // Bad: unwrap in library code
        let config_str = std::fs::read_to_string("config.toml").unwrap();

        Ok(Config {
            name: "app".to_string(),
            version: "1.0".to_string(),
        })
    }

    fn get_setting(&self, key: &str) -> String {
        // Bad: expect with custom message but still panics
        self.get_optional_setting(key).expect("Setting must exist")
    }

    fn get_optional_setting(&self, _key: &str) -> Option<String> {
        None
    }
}
